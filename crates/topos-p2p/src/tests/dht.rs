use futures::StreamExt;
use libp2p::{
    kad::{record::Key, KademliaEvent, PutRecordOk, QueryResult, Record},
    swarm::SwarmEvent,
};
use rstest::rstest;
use test_log::test;

use crate::{
    event::ComposedEvent, network::NetworkBuilder, tests::support::local_peer, wait_for_event,
    Client, Runtime,
};

use super::support::{dummy_peer, PeerAddr};

#[rstest]
#[test(tokio::test)]
#[ignore = "need to be fixed"]
async fn put_value_in_dht(#[future] dummy_peer: (Client, PeerAddr)) {
    let (_client, dummy_peer) = dummy_peer.await;

    let (key, addr) = local_peer(2);
    println!("multi addr: {}", addr);
    let (_client, _stream, mut runtime): (_, _, Runtime) = NetworkBuilder::default()
        .peer_key(key)
        .known_peers(&[dummy_peer])
        .listen_addr(addr.clone())
        .exposed_addresses(addr)
        .build()
        .await
        .unwrap();

    let kad = &mut runtime.swarm.behaviour_mut().discovery;

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
