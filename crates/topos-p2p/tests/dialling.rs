mod support {
    pub mod network;
}

use crate::support::network::{local_peer, TestNodeContext};
use serde::{Deserialize, Serialize};
use std::time::Duration;
use support::network::{local_peers, start_node};
use test_log::test;
use tokio::time::sleep;
use topos_p2p::RetryPolicy;

macro_rules! wait_for_event {
    ($node:ident, matches: $(|)? $( $pattern:pat_param )|+ $( if $guard: expr )? $(,)?) => {
        let assertion = async {
            while let Some(event) = $node.next_event().await {
                if matches!(event, $( $pattern )|+ $( if $guard )?) {
                    break;
                }
            }
        };

        if let Err(_) = tokio::time::timeout(std::time::Duration::from_millis(100), assertion).await
        {
            panic!("Timeout waiting for event");
        }
    };
}

#[test(tokio::test)]
async fn dialling_between_3_peers() {
    let peer_configs = local_peers(3);
    let all_peers = peer_configs.iter().fold(Vec::new(), |mut out, item| {
        out.push(item.to_owned());
        out
    });

    let tasks: Vec<_> = peer_configs
        .into_iter()
        .map(|peer| start_node(peer.to_owned(), vec![]))
        .collect();

    let mut nodes: Vec<TestNodeContext> = Vec::new();

    for task in tasks {
        nodes.push(task.await);
    }

    assert_eq!(nodes.len(), 3);

    for node in &nodes {
        for (key, _port, addr) in all_peers.to_owned() {
            let _ = node.client.dial(key.public().to_peer_id(), addr).await;
        }
    }

    for node in &nodes {
        assert_eq!(
            2,
            node.client
                .connected_peers()
                .await
                .map(|list| list.len())
                .unwrap()
        );
    }

    let mut drained_node = nodes.drain(..1).collect::<Vec<_>>();
    let first_node = drained_node.first_mut().unwrap();
    let first_node_peer_id = first_node.peer_id;

    let _ = first_node
        .client
        .disconnect()
        .await
        .expect("Unable to stop node");

    // First client is disconnected
    assert_eq!(
        0,
        first_node
            .client
            .connected_peers()
            .await
            .map(|list| list.len())
            .unwrap()
    );

    let other_nodes_peer_ids = nodes.iter().map(|node| node.peer_id).collect::<Vec<_>>();

    wait_for_event!(
        first_node,
        matches: topos_p2p::Event::PeerDisconnected { peer_id } if other_nodes_peer_ids.contains(&peer_id)
    );

    wait_for_event!(
        first_node,
        matches: topos_p2p::Event::PeerDisconnected { peer_id } if other_nodes_peer_ids.contains(&peer_id)
    );

    let iterator = nodes.iter_mut().skip(1);

    for node in iterator {
        assert_eq!(
            1,
            node.client
                .connected_peers()
                .await
                .map(|list| list.len())
                .unwrap()
        );

        wait_for_event!(
            node,
            matches: topos_p2p::Event::PeerDisconnected { peer_id } if peer_id == first_node_peer_id
        );
    }
}

#[test(tokio::test)]
async fn request_response() {
    let peer_1 = local_peer(1);
    let peer_2 = local_peer(2);
    let known_peers_1 = vec![];
    let known_peers_2 = vec![(peer_2.0.public().to_peer_id(), peer_2.2.clone())];

    let mut receiver = start_node(peer_1, known_peers_1).await;
    let sender = start_node(peer_2, known_peers_2).await;

    let _ = sender
        .client
        .dial(receiver.peer_id, receiver.peer_addr.clone())
        .await;

    assert_eq!(
        1,
        sender
            .client
            .connected_peers()
            .await
            .map(|list| list.len())
            .unwrap()
    );
    let receiver_peer = receiver.peer_id;
    let expected_data = TceCommands::Response(Ok(()));
    let response_data = expected_data.clone();

    let join_receive = tokio::spawn(Box::pin(async move {
        if let Some(topos_p2p::Event::TransmissionOnReq { channel, .. }) =
            receiver.next_event().await
        {
            let _ = receiver
                .client
                .respond_to_request(response_data, channel)
                .await;

            // NOTE: Hack to wait for the receiver to read the stream, if not, the stream is closed before reading the response.
            // Maybe something more elegant can be done but I didn't find a solution for now
            sleep(Duration::from_millis(100)).await;
        }
    }));

    let join_send = tokio::spawn(Box::pin(async move {
        let x = sender
            .client
            .send_request::<_, TceCommands>(
                receiver_peer,
                TceCommands::OnEchoSubscribeReq {
                    from_peer: sender.peer_id.to_base58(),
                },
                RetryPolicy::NoRetry,
            )
            .await;

        assert_eq!(x.unwrap(), expected_data);
    }));

    futures::future::join_all(vec![join_receive, join_send]).await;
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub enum TceCommands {
    /// Given peer sent EchoSubscribe request
    OnEchoSubscribeReq {
        from_peer: String,
    },
    Response(Result<(), ()>),
}

impl From<TceCommands> for Vec<u8> {
    fn from(cmd: TceCommands) -> Self {
        bincode::serialize(&cmd).expect("Can't serialize")
    }
}

impl From<Vec<u8>> for TceCommands {
    fn from(data: Vec<u8>) -> Self {
        bincode::deserialize(data.as_ref()).expect("Can't Deserialize")
    }
}
