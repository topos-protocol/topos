use futures::{future::join_all, StreamExt};
use libp2p::PeerId;
use rand::seq::IteratorRandom;
use rstest::*;
use std::collections::{HashMap, HashSet};
use std::time::Duration;
use test_log::test;
use tokio::spawn;
use tokio::sync::mpsc;
use topos_core::{
    api::grpc::{
        shared::v1::checkpoints::TargetCheckpoint,
        tce::v1::{
            watch_certificates_request::OpenStream,
            watch_certificates_response::{CertificatePushed, Event},
            StatusRequest, SubmitCertificateRequest,
        },
    },
    uci::{Certificate, SubnetId, SUBNET_ID_LENGTH},
};
use topos_test_sdk::{certificates::create_certificate_chains, tce::create_network};
use tracing::{debug, error, info, warn};

const NUMBER_OF_SUBNETS_PER_CLIENT: usize = 1;

fn get_subset_of_subnets(subnets: &[SubnetId], subset_size: usize) -> Vec<SubnetId> {
    let mut rng = rand::thread_rng();
    Vec::from_iter(
        subnets
            .iter()
            .cloned()
            .choose_multiple(&mut rng, subset_size),
    )
}

#[rstest]
#[test(tokio::test)]
#[timeout(Duration::from_secs(5))]
async fn start_a_cluster() {
    let mut peers_context = create_network(5, vec![]).await;

    let mut status: Vec<bool> = Vec::new();

    for (_peer_id, client) in peers_context.iter_mut() {
        let response = client
            .console_grpc_client
            .status(StatusRequest {})
            .await
            .expect("Can't get status");

        status.push(response.into_inner().has_active_sample);
    }

    assert!(status.iter().all(|s| *s));
}

#[rstest]
#[tokio::test]
#[timeout(Duration::from_secs(30))]
// FIXME: This test is flaky, it fails sometimes because of gRPC connection error (StreamClosed)
async fn cert_delivery() {
    let subscriber = tracing_subscriber::FmtSubscriber::builder()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .with_test_writer()
        .finish();
    let _ = tracing::subscriber::set_global_default(subscriber);

    let peer_number = 5;
    let number_of_certificates_per_subnet = 2;
    let number_of_subnets = 3;

    let all_subnets: Vec<SubnetId> = (1..=number_of_subnets)
        .map(|v| [v as u8; SUBNET_ID_LENGTH].into())
        .collect();

    // Generate certificates with respect to parameters
    let mut subnet_certificates =
        create_certificate_chains(all_subnets.as_ref(), number_of_certificates_per_subnet)
            .into_iter()
            .map(|(s, v)| (s, v.into_iter().map(|v| v.certificate).collect::<Vec<_>>()))
            .collect::<HashMap<_, _>>();

    debug!(
        "Generated certificates for distribution per subnet: {:#?}",
        &subnet_certificates
    );

    // Calculate expected final set of delivered certificates (every subnet  should receive certificates that has cross
    // chain transaction targeting it)
    let mut expected_certificates: HashMap<SubnetId, HashSet<Certificate>> = HashMap::new();
    for certificates in subnet_certificates.values() {
        for cert in certificates {
            for target_subnet in &cert.target_subnets {
                expected_certificates
                    .entry(*target_subnet)
                    .or_default()
                    .insert(cert.clone());
            }
        }
    }

    warn!("Starting the cluster...");
    // List of peers (tce nodes) with their context
    let mut peers_context = create_network(peer_number, vec![]).await;

    warn!("Cluster started, starting clients...");
    // Connected tce clients are passing received certificates to this mpsc::Receiver, collect all of them
    let mut clients_delivered_certificates: Vec<mpsc::Receiver<(PeerId, SubnetId, Certificate)>> =
        Vec::new(); // (Peer Id, Subnet Id, Certificate)
    let mut client_tasks: Vec<tokio::task::JoinHandle<()>> = Vec::new(); // Clients connected to TCE API Service run in async tasks

    let mut assign_at_least_one_client_to_every_subnet = all_subnets.clone();
    for (peer_id, ctx) in peers_context.iter_mut() {
        let peer_id = *peer_id;
        // Make sure that every subnet is represented (connected through client) to at least 1 peer
        // After that assign subnets randomly to clients, 1 subnet per connection to TCE
        // as it is assumed that NUMBER_OF_SUBNETS_PER_CLIENT is 1 - that is also realistic case, topos node representing one subnet
        let client_subnet_id: SubnetId = if assign_at_least_one_client_to_every_subnet.is_empty() {
            get_subset_of_subnets(&all_subnets, NUMBER_OF_SUBNETS_PER_CLIENT).remove(0)
        } else {
            assign_at_least_one_client_to_every_subnet.pop().unwrap()
        };

        // Number of subnets one client is representing, normally 1
        ctx.connected_subnets = Some(vec![client_subnet_id]);
        debug!(
            "Opening client for peer id: {} with subnet_ids: {:?}",
            &peer_id, &client_subnet_id,
        );

        // (Peer id, Subnet Id, Certificate)
        let (tx, rx) = mpsc::channel::<(PeerId, SubnetId, Certificate)>(
            number_of_certificates_per_subnet * number_of_subnets,
        );
        clients_delivered_certificates.push(rx);

        let in_stream_subnet_id = client_subnet_id;
        let in_stream = async_stream::stream! {
            yield OpenStream {
                target_checkpoint: Some(TargetCheckpoint{
                    target_subnet_ids: vec![in_stream_subnet_id.into()],
                    positions: Vec::new()
                }),
                source_checkpoint: None
            }.into();
        };

        // Number of certificates expected to receive for every subnet,
        // to know when to close the TCE clients (and finish test)
        let mut incoming_certificates_number =
            expected_certificates.get(&client_subnet_id).unwrap().len();
        // Open client connection to TCE service in separate async tasks
        let mut client = ctx.api_grpc_client.clone();
        let expected_certificate_debug: Vec<_> = expected_certificates
            .get(&client_subnet_id)
            .unwrap()
            .iter()
            .map(|c| c.id)
            .collect();

        let response = client.watch_certificates(in_stream).await.unwrap();

        let client_task = spawn(async move {
            debug!(
                "Spawning client task for peer: {} waiting for {} certificates: {:?}",
                peer_id, incoming_certificates_number, expected_certificate_debug
            );

            let mut resp_stream = response.into_inner();
            while let Some(received) = resp_stream.next().await {
                let received = received.unwrap();

                if let Some(Event::CertificatePushed(CertificatePushed {
                    certificate: Some(certificate),
                    ..
                })) = received.event
                {
                    debug!(
                        "Client peer_id: {} certificate id: {} delivered to subnet id {}, ",
                        &peer_id,
                        certificate.id.clone().unwrap(),
                        &client_subnet_id
                    );
                    tx.send((peer_id, client_subnet_id, certificate.try_into().unwrap()))
                        .await
                        .unwrap();
                    incoming_certificates_number -= 1;
                    if incoming_certificates_number == 0 {
                        // We have received all expected certificates for this subnet, end client
                        break;
                    }
                }
            }
            debug!("Finishing client for peer_id: {}", &peer_id);
        });
        client_tasks.push(client_task);
    }

    info!(
        "Waiting for expected delivered certificates {:?}",
        expected_certificates
    );
    // Delivery tasks collect certificates that clients of every TCE node
    // are receiving to reduce them to one channel (delivery_rx)
    let mut delivery_tasks = Vec::new();
    // delivery_tx/delivery_rx Pass certificates from delivery tasks of every client to final collection of delivered certificates
    let (delivery_tx, mut delivery_rx) = mpsc::channel::<(PeerId, SubnetId, Certificate)>(
        peer_number * number_of_certificates_per_subnet * number_of_subnets,
    );
    for (index, mut client_delivered_certificates) in
        clients_delivered_certificates.into_iter().enumerate()
    {
        let delivery_tx = delivery_tx.clone();
        let delivery_task = tokio::spawn(async move {
            // Read certificates that every client has received
            info!("Delivery task for receiver {}", index);
            loop {
                let x = client_delivered_certificates.recv().await;

                match x {
                    Some((peer_id, target_subnet_id, cert)) => {
                        info!(
                            "Delivered certificate on peer_Id: {} cert id: {} from source subnet \
                             id: {} to target subnet id {}",
                            &peer_id, cert.id, cert.source_subnet_id, target_subnet_id
                        );
                        // Send certificates from every peer to one delivery_rx receiver
                        delivery_tx
                            .send((peer_id, target_subnet_id, cert))
                            .await
                            .unwrap();
                    }
                    _ => break,
                }
            }
            // We will end this loop when sending TCE client has dropped channel sender and there
            // are not certificates in channel
            info!("End delivery task for receiver {}", index);
        });
        delivery_tasks.push(delivery_task);
    }
    drop(delivery_tx);

    // Broadcast multiple certificates from all subnets
    info!("Broadcasting certificates...");
    for (peer_id, client) in peers_context.iter_mut() {
        // If there exist of connected subnets to particular TCE
        if let Some(ref connected_subnets) = client.connected_subnets {
            // Iterate all subnets connected to TCE (normally 1)
            for subnet_id in connected_subnets {
                if let Some(certificates) = subnet_certificates.get_mut(subnet_id) {
                    // Iterate all certificates meant to be sent to the particular network
                    for cert in certificates.iter() {
                        info!(
                            "Sending certificate id={} from subnet id: {} to peer id: {}",
                            &cert.id, &subnet_id, &peer_id
                        );
                        let _ = client
                            .api_grpc_client
                            .submit_certificate(SubmitCertificateRequest {
                                certificate: Some(cert.clone().into()),
                            })
                            .await
                            .expect("Can't send certificate");
                    }
                    // Remove sent certificate, every certificate is sent only once to TCE network
                    certificates.clear();
                }
            }
        }
    }
    let assertion = async move {
        info!("Waiting for all delivery tasks");
        join_all(delivery_tasks).await;
        info!("All expected clients delivered");
        let mut delivered_certificates: HashMap<PeerId, HashMap<SubnetId, HashSet<Certificate>>> =
            HashMap::new();
        // Collect all certificates per peer_id and subnet_id
        while let Some((peer_id, receiving_subnet_id, cert)) = delivery_rx.recv().await {
            debug!("Counting delivered certificate cert id: {:?}", cert.id);
            delivered_certificates
                .entry(peer_id)
                .or_default()
                .entry(receiving_subnet_id)
                .or_default()
                .insert(cert);
        }
        info!("All incoming certificates received");
        // Check received certificates for every peer and every subnet
        for delivered_certificates_per_peer in delivered_certificates.values() {
            for (subnet_id, delivered_certificates_per_subnet) in delivered_certificates_per_peer {
                assert_eq!(
                    expected_certificates.get(subnet_id).unwrap().len(),
                    delivered_certificates_per_subnet.len()
                );
                assert_eq!(
                    expected_certificates.get(subnet_id).unwrap(),
                    delivered_certificates_per_subnet
                );
            }
        }
    };

    // Set big timeout to prevent flaky fails. Instead fail/panic early in the test to indicate actual error
    if tokio::time::timeout(std::time::Duration::from_secs(30), assertion)
        .await
        .is_err()
    {
        panic!("Timeout waiting for command");
    }
}
