mod support {
    pub mod certificate;
    pub mod network;
}

use crate::support::certificate::generate_cert;
use futures::{future::join_all, StreamExt};
use rand::seq::IteratorRandom;
use std::collections::{HashMap, HashSet};
use test_log::test;
use tokio::spawn;
use tokio::sync::mpsc;
use topos_core::{
    api::tce::v1::{
        watch_certificates_request::OpenStream,
        watch_certificates_response::{CertificatePushed, Event},
    },
    uci::Certificate,
};
use topos_tce_broadcast::{uci::SubnetId, DoubleEchoCommand, SamplerCommand};
use tracing::{debug, info};

fn get_subset_of_subnets(subnets: &[SubnetId], subset_size: usize) -> Vec<SubnetId> {
    let mut rng = rand::thread_rng();
    Vec::from_iter(
        subnets
            .iter()
            .cloned()
            .choose_multiple(&mut rng, subset_size),
    )
}

#[test(tokio::test)]
async fn cert_delivery() {
    let peer_number = 10;
    let correct_sample = 4;
    let number_of_certificates_per_subnet = 2;
    let number_of_subnets = 3;
    const NUMBER_OF_SUBNETS_PER_CLIENT: usize = 1; // In real life this would be always 1, topos node would represent one subnet

    let g = |a, b| (((a as f32) * b) as f32).ceil() as usize;

    let all_subnets: Vec<SubnetId> = (1..=number_of_subnets).map(|v| v.to_string()).collect();

    // Clients connected to TCE API Service
    let mut clients = support::network::start_peer_pool(peer_number as u8, correct_sample, g).await;

    // Generate certificates with respect to parameters
    let mut subnet_certificates = generate_cert(&all_subnets, number_of_certificates_per_subnet);
    debug!(
        "Generated certificates for distribution per subnet: {:#?}",
        &subnet_certificates
    );

    // Calculate expected final set of delivered certificates
    let mut expected_certificates: HashMap<SubnetId, HashSet<Certificate>> = HashMap::new();
    for (_source_subnet_id, certificates) in &subnet_certificates {
        for cert in certificates {
            for cross_chain_tx in &cert.calls {
                expected_certificates
                    .entry(cross_chain_tx.terminal_subnet_id.clone())
                    .or_insert(HashSet::new())
                    .insert(cert.clone());
            }
        }
    }
    // Connected tce clients are passing received certificates to this mpsc::Receiver, collect all of them
    let mut clients_delivered_certificates: Vec<mpsc::Receiver<(String, String, Certificate)>> =
        Vec::new(); // (Peer Id, Subnet Id, Certificate)
    let mut client_tasks: Vec<tokio::task::JoinHandle<()>> = Vec::new(); // Clients connected to TCE API Service run in async tasks

    let mut assign_at_least_one_client_to_every_subnet = all_subnets.clone();
    for (peer_id, ctx) in clients.iter_mut() {
        let peer_id = peer_id.clone();
        // Make sure that every subnet is represented (connected through client) to at least 1 peer
        // After that assign subnets randomly to clients, 1 subnet per connection to TCE
        // as it is assumed that NUMBER_OF_SUBNETS_PER_CLIENT is 1 - that is also realistic case, topos node representing one subnet
        let client_subnet_id: SubnetId = if assign_at_least_one_client_to_every_subnet.is_empty() {
            get_subset_of_subnets(&all_subnets, NUMBER_OF_SUBNETS_PER_CLIENT).remove(0)
        } else {
            assign_at_least_one_client_to_every_subnet.pop().unwrap()
        };

        // Number of subnets one client is representing, normally 1
        ctx.connected_subnets = Some(vec![client_subnet_id.clone()]);
        debug!(
            "Opening client for peer id: {} with subnet_ids: {:?}",
            &peer_id, &client_subnet_id,
        );

        // (Peer id, Subnet Id, Certificate)
        let (tx, rx) = mpsc::channel::<(String, SubnetId, Certificate)>(
            number_of_certificates_per_subnet * number_of_subnets,
        );
        clients_delivered_certificates.push(rx);

        let in_stream_subnet_id = client_subnet_id.clone();
        let in_stream = async_stream::stream! {
            yield OpenStream { subnet_ids: vec![in_stream_subnet_id.into()] }.into();
        };

        // Number of certificates expected to receive for every subnet,
        // to know when to close the TCE clients (and finish test)
        let mut incoming_certificates_number =
            expected_certificates.get(&client_subnet_id).unwrap().len();
        // Open client connection to TCE service in separate async tasks
        if let Some(mut client) = ctx.api_grpc_client.take() {
            let client_subnet_id = client_subnet_id.clone();
            let client_task = spawn(async move {
                debug!("Spawning client task for peer: {}", peer_id);
                let response = client.watch_certificates(in_stream).await.unwrap();

                let mut resp_stream = response.into_inner();
                while let Some(received) = resp_stream.next().await {
                    let received = received.unwrap();
                    if let Some(Event::CertificatePushed(CertificatePushed {
                        certificate: Some(certificate),
                    })) = received.event
                    {
                        debug!("Client peer_id: {} certificate cert_id: {} delivered to subnet id {}, ",
                            &peer_id, &certificate.cert_id, &client_subnet_id);
                        tx.send((
                            peer_id.clone(),
                            client_subnet_id.clone().into(),
                            certificate.into(),
                        ))
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
    }

    // NOTE: Needed time for nodes to put their record on DHT
    // plus the resolution from other peers
    info!("Waiting proper peer resolution");
    tokio::time::sleep(std::time::Duration::from_secs(3)).await;

    // Force TCE nodes to recreate subscriptions and subscribers
    info!("Trigger the new network view");
    for i in 0..clients.len() {
        let current_peer = format!("peer_{i}");
        if let Some(client) = clients.get(&current_peer) {
            let _ = client
                .command_sampler
                .send(SamplerCommand::PeersChanged {
                    peers: clients
                        .iter()
                        .filter_map(|(key, ctx)| {
                            if key == &current_peer {
                                None
                            } else {
                                Some(ctx.peer_id.to_string())
                            }
                        })
                        .collect::<Vec<_>>()
                        .clone(),
                })
                .await
                .expect("Can't send certificate");
        }
    }
    // Waiting for new network view
    tokio::time::sleep(std::time::Duration::from_secs(4)).await;

    // Broadcast multiple certificates on every subnet
    info!("Broadcasting certificates...");
    for (peer_id, client) in clients {
        // If there exist of connected subnets to particular TCE
        if let Some(connected_subnets) = client.connected_subnets {
            // Iterate all subnets connected to TCE (normally 1)
            for subnet_id in connected_subnets {
                if let Some(certificates) = subnet_certificates.get_mut(&subnet_id) {
                    // Iterate all certificates meant to be sent to the particular network
                    for cert in certificates.iter() {
                        debug!(
                            "Sending certificate id={} from subnet id: {} to peer id: {}",
                            &cert.cert_id, &subnet_id, &peer_id
                        );
                        let _ = client
                            .command_broadcast
                            .send(DoubleEchoCommand::Broadcast { cert: cert.clone() })
                            .await
                            .expect("Can't send certificate");
                    }
                    // Remove sent certificate, every certificate is sent only once to TCE network
                    certificates.clear();
                }
            }
        }
    }

    info!(
        "Waiting for expected delivered certificates {:#?}",
        expected_certificates
    );
    // Delivery tasks collect certificates that clients of every TCE node
    // are receiving to reduce them to one channel (delivery_rx)
    let mut delivery_tasks = Vec::new();
    // delivery_tx/delivery_rx Pass certificates from delivery tasks of every client to final collection of delivered certificates
    let (delivery_tx, mut delivery_rx) = mpsc::channel::<(String, SubnetId, Certificate)>(
        peer_number * number_of_certificates_per_subnet * number_of_subnets,
    );
    for (index, mut client_delivered_certificates) in
        clients_delivered_certificates.into_iter().enumerate()
    {
        let delivery_tx = delivery_tx.clone();
        let delivery_task = tokio::spawn(async move {
            // Read certificates that every client has received
            debug!("Delivery task for receiver {}", index);
            while let Some((peer_id, receiving_subnet_id, cert)) =
                client_delivered_certificates.recv().await
            {
                debug!(
                    "Delivered certificate on peer_Id: {} cert id: {} from initial subnet id: {} to target subnet id {}",
                    &peer_id, cert.cert_id, cert.initial_subnet_id, receiving_subnet_id
                );
                // Send certificates from every peer to one delivery_rx receiver
                delivery_tx
                    .send((peer_id, receiving_subnet_id, cert))
                    .await
                    .unwrap();
            }
            // We will end this loop when sending TCE client has dropped channel sender and there
            // are not certificates in channel
            debug!("End delivery task for receiver {}", index);
        });
        delivery_tasks.push(delivery_task);
    }
    drop(delivery_tx);

    let assertion = async move {
        info!("Waiting for all delivery tasks");
        join_all(delivery_tasks).await;
        info!("All expected clients delivered");
        let mut delivered_certificates: HashMap<String, HashMap<SubnetId, HashSet<Certificate>>> =
            HashMap::new();
        // Collect all certificates per peer_id and subnet_id
        while let Some((peer_id, receiving_subnet_id, cert)) = delivery_rx.recv().await {
            debug!("Counting delivered certificate cert id: {}", cert.cert_id);
            delivered_certificates
                .entry(peer_id)
                .or_insert(HashMap::new())
                .entry(receiving_subnet_id)
                .or_insert(HashSet::new())
                .insert(cert);
        }
        info!("All incoming certificates received");
        // Check received certificates for every peer and every subnet
        for (_peer, delivered_certificates_per_peer) in &delivered_certificates {
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
    if let Err(_) = tokio::time::timeout(std::time::Duration::from_secs(8), assertion).await {
        panic!("Timeout waiting for command");
    }
}

pub fn sample_lower_bound(n_u: usize) -> usize {
    let k: f32 = 2.;
    (n_u as f32).log(k) as usize
}
