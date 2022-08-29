mod support {
    pub mod certificate;
    pub mod network;
}

use crate::support::certificate::generate_cert;
use std::time::Duration;
use tce_transport::TrbpCommands;
use tce_trbp::{uci::SubnetId, DoubleEchoCommand};
use test_log::test;

#[test(tokio::test)]
async fn cert_delivery() {
    let peer_number = 5;
    let correct_sample = 3;

    let g = |a, b| (((a as f32) * b) as f32).ceil() as usize;

    let all_subnets: Vec<SubnetId> = (1..=10).collect();

    let clients = support::network::start_peer_pool(peer_number as u8, correct_sample, g).await;

    let cert = generate_cert(&all_subnets, 1);

    if let Some(client) = clients.get("peer_1") {
        let _ = client
            .command_broadcast
            .send(DoubleEchoCommand::Command(TrbpCommands::OnBroadcast {
                cert: cert.first().cloned().unwrap(),
            }))
            .await
            .expect("Can't send certificate");
    }

    tokio::time::sleep(Duration::from_secs(1)).await;

    // Here to be able to display logs
    // assert!(false);
}

pub fn sample_lower_bound(n_u: usize) -> usize {
    let k: f32 = 2.;
    (n_u as f32).log(k) as usize
}
