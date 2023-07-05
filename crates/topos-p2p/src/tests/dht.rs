use std::{num::NonZeroUsize, time::Duration};

use futures::StreamExt;
use libp2p::{
    identify::{self, Info},
    kad::{record::Key, KademliaEvent, PutRecordOk, QueryResult, Record},
    swarm::SwarmEvent,
};
use rstest::rstest;
use test_log::test;
use topos_test_sdk::tce::NodeConfig;

use crate::{
    config::DiscoveryConfig, event::ComposedEvent, network::NetworkBuilder,
    tests::support::local_peer, wait_for_event, Client, Runtime,
};

use super::support::{dummy_peer, PeerAddr};

#[rstest]
#[test(tokio::test)]
#[timeout(Duration::from_secs(5))]
async fn put_value_in_dht() {
    let peer_1 = NodeConfig::from_seed(1);
    let peer_2 = NodeConfig::from_seed(2);

    let (_client, _, join) = peer_1.bootstrap(&[]).await.unwrap();

    let (_, _, runtime) = crate::network::builder()
        .peer_key(peer_2.keypair.clone())
        .known_peers(&[(peer_1.peer_id(), peer_1.addr.clone())])
        .exposed_addresses(peer_2.addr.clone())
        .listen_addr(peer_2.addr.clone())
        .minimum_cluster_size(1)
        .discovery_config(
            DiscoveryConfig::default().with_replication_factor(NonZeroUsize::new(1).unwrap()),
        )
        .build()
        .await
        .expect("Unable to create p2p network");

    let mut runtime = runtime.bootstrap().await.unwrap();
    // runtime
    //     .swarm
    //     .behaviour_mut()
    //     .discovery
    //     .inner
    //     .add_address(&peer_1.keypair.public().to_peer_id(), peer_1.addr);

    // loop {
    //     if let Some(SwarmEvent::Behaviour(ComposedEvent::PeerInfo(event))) =
    //         runtime.swarm.next().await
    //     {
    //         println!("Event: {:?}", event);
    //         if let identify::Event::Received {
    //             peer_id,
    //             info: Info { observed_addr, .. },
    //         } = *event
    //         {
    //             println!("Peer {} observed address: {}", peer_id, observed_addr);
    //             runtime.swarm.add_external_address(observed_addr);
    //             break;
    //         }
    //     }
    // }

    let kad = &mut runtime.swarm.behaviour_mut().discovery;

    let input_key = Key::new(&runtime.local_peer_id.to_string());
    _ = kad
        .inner
        .put_record(
            Record::new(input_key.clone(), runtime.addresses.to_vec()),
            libp2p::kad::Quorum::One,
        )
        .unwrap();

    let mut swarm = runtime.swarm;

    wait_for_event!(
        swarm,
        matches: SwarmEvent::Behaviour(ComposedEvent::Kademlia(kademlia_event)) if matches!(&*kademlia_event, KademliaEvent::OutboundQueryProgressed { result: QueryResult::PutRecord(Ok(PutRecordOk { key: input_key })), .. })
    );

    join.abort();
}

#[test(tokio::test)]
#[ignore = "not implemented yet"]
async fn fetch_value_from_dht() {}

#[test(tokio::test)]
#[ignore = "not implemented yet"]
async fn remove_value_from_dht() {}

#[test(tokio::test)]
#[ignore = "not implemented yet"]
async fn peer_read_an_available_value() {}

#[test(tokio::test)]
#[ignore = "not implemented yet"]
async fn peer_read_an_unavailable_value() {}

#[test(tokio::test)]
#[ignore = "not implemented yet"]
async fn reading_available_data_with_client() {}

#[test(tokio::test)]
#[ignore = "not implemented yet"]
async fn reading_unavailable_data_with_client() {}
