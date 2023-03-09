use std::time::Duration;

use futures::StreamExt;
use libp2p::{
    kad::{record::Key, KademliaEvent, PutRecordOk, QueryResult, Record},
    swarm::SwarmEvent,
};
use rstest::rstest;
use test_log::test;
use topos_test_sdk::tce::NodeConfig;

use crate::{
    event::ComposedEvent, network::NetworkBuilder, tests::support::local_peer, wait_for_event,
    Client, Runtime,
};

use super::support::{dummy_peer, PeerAddr};

#[rstest]
#[test(tokio::test)]
#[timeout(Duration::from_secs(1))]
async fn put_value_in_dht() {
    let peer_1 = NodeConfig::from_seed(1);
    let peer_2 = NodeConfig::from_seed(2);

    let (_client, _, join) = peer_1.bootstrap(&[]).await.unwrap();

    let (_, _, runtime) = crate::network::builder()
        .peer_key(peer_2.keypair.clone())
        .known_peers(&[])
        .exposed_addresses(peer_2.addr.clone())
        .listen_addr(peer_2.addr.clone())
        .minimum_cluster_size(0)
        .build()
        .await
        .expect("Unable to create p2p network");

    let mut runtime = runtime.bootstrap().await.unwrap();

    let kad = &mut runtime.swarm.behaviour_mut().discovery;
    kad.add_address(&peer_1.keypair.public().to_peer_id(), peer_1.addr);

    let input_key = Key::new(&runtime.local_peer_id.to_string());
    _ = kad
        .put_record(
            Record::new(input_key.clone(), runtime.addresses.to_vec()),
            libp2p::kad::Quorum::One,
        )
        .unwrap();

    let mut swarm = runtime.swarm;

    wait_for_event!(
        swarm,
        matches: SwarmEvent::Behaviour(ComposedEvent::Kademlia(KademliaEvent::OutboundQueryProgressed { result: QueryResult::PutRecord(Ok(PutRecordOk { key })), .. } )) if key == input_key
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
