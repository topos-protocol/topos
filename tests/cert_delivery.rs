mod support {
    pub mod certificate;
    pub mod network;
}

use crate::support::certificate::generate_cert;
//use tce_transport::TrbpCommands;
use test_log::test;
use topos_tce_broadcast::{uci::SubnetId, DoubleEchoCommand, SamplerCommand};
use tracing::info;

#[test(tokio::test)]
async fn cert_delivery() {
    let peer_number = 5;
    let correct_sample = 2;

    let g = |a, b| (((a as f32) * b) as f32).ceil() as usize;

    let all_subnets: Vec<SubnetId> = (1..=2).map(|v| v.to_string()).collect();

    let clients = support::network::start_peer_pool(peer_number as u8, correct_sample, g).await;

    let cert = generate_cert(&all_subnets, 1);

    let all_peer_addr = clients
        .iter()
        .map(|(_, ctx)| ctx.peer_id.to_string())
        .collect::<Vec<_>>();

    // NOTE: Needed time for nodes to put their record on DHT
    // plus the resolution from other peers
    info!("Waiting proper peer discovery");
    tokio::time::sleep(std::time::Duration::from_secs(4)).await;

    info!("Trigger the new network view");
    for i in 0..clients.len() {
        if let Some(client) = clients.get(&format!("peer_{i}")) {
            let _ = client
                .command_sampler
                .send(SamplerCommand::PeersChanged {
                    peers: all_peer_addr.clone(),
                })
                .await
                .expect("Can't send certificate");
        }
    }

    tokio::time::sleep(std::time::Duration::from_secs(20)).await;

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

    // tokio::time::sleep(std::time::Duration::from_secs(10)).await;

    // FIXME: assert properly
}

pub fn sample_lower_bound(n_u: usize) -> usize {
    let k: f32 = 2.;
    (n_u as f32).log(k) as usize
}
