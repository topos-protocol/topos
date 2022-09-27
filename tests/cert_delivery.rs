mod support {
    pub mod certificate;
    pub mod network;
}

use crate::support::certificate::generate_cert;
use futures::{future::join_all, StreamExt};
//use tce_transport::TrbpCommands;
use test_log::test;
use tokio::{spawn, sync::oneshot};
use topos_core::{
    api::tce::v1::{
        watch_certificates_request::OpenStream,
        watch_certificates_response::{CertificatePushed, Event},
    },
    uci::Certificate,
};
use topos_tce_broadcast::{uci::SubnetId, DoubleEchoCommand, SamplerCommand};
use tracing::info;

#[test(tokio::test)]
async fn cert_delivery() {
    let peer_number = 5;
    let correct_sample = 2;

    let g = |a, b| (((a as f32) * b) as f32).ceil() as usize;

    let all_subnets: Vec<SubnetId> = (1..=2).map(|v| v.to_string()).collect();

    let mut clients = support::network::start_peer_pool(peer_number as u8, correct_sample, g).await;

    let cert = generate_cert(&all_subnets, 1);

    let mut expected_delivery: Vec<oneshot::Receiver<Certificate>> = Vec::new();

    for (_key, ctx) in clients.iter_mut().filter(|(k, _)| *k != "peer_1") {
        let (tx, rx) = oneshot::channel::<Certificate>();
        expected_delivery.push(rx);

        let in_stream = async_stream::stream! {
            yield OpenStream { subnet_ids: vec!["1".into(), "2".into()] }.into();
        };

        if let Some(mut client) = ctx.api_grpc_client.take() {
            spawn(async move {
                let response = client.watch_certificates(in_stream).await.unwrap();

                let mut resp_stream = response.into_inner();

                let mut tx = Some(tx);
                while let Some(received) = resp_stream.next().await {
                    let received = received.unwrap();
                    if let Some(Event::CertificatePushed(CertificatePushed {
                        certificate: Some(certificate),
                    })) = received.event
                    {
                        if let Some(tx) = tx.take() {
                            _ = tx.send(certificate.into());
                        } else {
                            panic!("Double certificate sent");
                        }
                    }
                }
            });
        }
    }

    // NOTE: Needed time for nodes to put their record on DHT
    // plus the resolution from other peers
    info!("Waiting proper peer discovery");
    tokio::time::sleep(std::time::Duration::from_secs(1)).await;

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

    tokio::time::sleep(std::time::Duration::from_secs(1)).await;

    // Broadcast one certificate
    info!("Broadcast 1 Certificate");
    if let Some(client) = clients.get("peer_1") {
        let _ = client
            .command_broadcast
            .send(DoubleEchoCommand::Broadcast {
                cert: cert.first().cloned().unwrap(),
            })
            .await
            .expect("Can't send certificate");
    }

    let expected_cert = cert.first().cloned().unwrap();

    let assertion = async {
        let results = join_all(expected_delivery).await;

        assert_eq!(results.len(), peer_number - 1);
        for result in results {
            assert!(result.is_ok());
            assert_eq!(result.unwrap(), expected_cert);
        }

        info!("Cert delivered to all GRPC client");
    };

    if let Err(_) = tokio::time::timeout(std::time::Duration::from_millis(300), assertion).await {
        panic!("Timeout waiting for command");
    }
    // FIXME: assert properly
}

pub fn sample_lower_bound(n_u: usize) -> usize {
    let k: f32 = 2.;
    (n_u as f32).log(k) as usize
}
