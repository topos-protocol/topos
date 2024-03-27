use std::{collections::HashSet, future::IntoFuture, time::Duration};

use libp2p::Swarm;
use libp2p_swarm_test::SwarmExt;
use rstest::rstest;
use test_log::test;
use tokio::spawn;
use tokio_util::sync::CancellationToken;
use tonic::server::NamedService;
use tonic::transport::Server;
use topos_test_sdk::grpc::{
    behaviour::{
        helloworld::{
            greeter_client::GreeterClient, greeter_server::GreeterServer, HelloRequest,
            HelloWithDelayRequest,
        },
        noop::noop_server::NoopServer,
    },
    implementations::{self, DummyServer},
};

use crate::{
    behaviour::grpc::{
        self,
        error::{OutboundConnectionError, OutboundError},
    },
    protocol_name, GrpcContext, GrpcRouter,
};

#[rstest]
#[test(tokio::test)]
#[timeout(Duration::from_secs(5))]
async fn instantiate_grpc() {
    let dummy = DummyServer {};
    let router = GrpcContext::default()
        .with_router(GrpcRouter::new(Server::builder()).add_service(GreeterServer::new(dummy)));

    let client_protocols = {
        let mut protocols = HashSet::new();

        protocols.insert(protocol_name!(GreeterServer::<DummyServer>::NAME));

        protocols
    };
    let mut client_swarm = Swarm::new_ephemeral(|_| grpc::Behaviour::new(GrpcContext::default()));
    let mut server_swarm = Swarm::new_ephemeral(|_| grpc::Behaviour::new(router));

    let server_peer_id = *server_swarm.local_peer_id();

    server_swarm.listen().await;

    let server_address = server_swarm.listeners().next().unwrap();

    client_swarm
        .behaviour_mut()
        .add_address(&server_peer_id, server_address.clone());

    let outbound_connection = client_swarm.behaviour_mut().open_outbound_connection(
        &server_peer_id,
        protocol_name!(GreeterServer::<DummyServer>::NAME),
    );

    let shutdown = CancellationToken::new();
    let client_shutdown = shutdown.child_token();
    let server_shutdown = shutdown.child_token();
    let client_swarm = async move {
        loop {
            tokio::select! {
                event = client_swarm.next_swarm_event() => {}
                _ = client_shutdown.cancelled() => { return client_swarm; }
            }
        }
    };

    let server_swarm = async move {
        loop {
            tokio::select! {
                _ = server_swarm.next_swarm_event() => {}
                _ = server_shutdown.cancelled() => { return server_swarm; }
            }
        }
    };

    let server_swarm = spawn(server_swarm);
    let client_swarm = spawn(client_swarm);
    println!("Starting");
    let connection = outbound_connection.into_future().await.unwrap();
    println!("Stopping");

    shutdown.cancel();

    let server_swarm = server_swarm.await.unwrap();
    let client_swarm = client_swarm.await.unwrap();

    assert_eq!(
        server_swarm.connected_peers().collect::<Vec<_>>(),
        vec![client_swarm.local_peer_id()]
    );
}

#[test(tokio::test)]
async fn opening_outbound_stream() {}

#[test(tokio::test)]
async fn opening_outbound_stream_half_close() {}

#[test(tokio::test)]
#[ignore = "TP-757: Need to find a way to properly close the connection after sending the query"]
async fn closing_stream() {
    let dummy = DummyServer {};

    let router = GrpcContext::default()
        .with_router(GrpcRouter::new(Server::builder()).add_service(GreeterServer::new(dummy)));
    let client_protocols = {
        let mut protocols = HashSet::new();

        protocols.insert(protocol_name!(GreeterServer::<DummyServer>::NAME));

        protocols
    };
    let mut client_swarm = Swarm::new_ephemeral(|_| grpc::Behaviour::new(GrpcContext::default()));
    let mut server_swarm = Swarm::new_ephemeral(|_| grpc::Behaviour::new(router));

    let server_peer_id = *server_swarm.local_peer_id();

    server_swarm.listen().await;

    let server_address = server_swarm.listeners().next().unwrap();
    client_swarm
        .behaviour_mut()
        .add_address(&server_peer_id, server_address.clone());

    let outbound_connection = client_swarm.behaviour_mut().open_outbound_connection(
        &server_peer_id,
        protocol_name!(GreeterServer::<DummyServer>::NAME),
    );

    let client_swarm = async move {
        loop {
            client_swarm.next_swarm_event().await;
        }
    };
    let server_swarm = async move {
        loop {
            server_swarm.next_swarm_event().await;
        }
    };

    spawn(server_swarm);
    spawn(client_swarm);
    let connection = outbound_connection.into_future().await.unwrap();

    let mut client = GreeterClient::new(connection.channel);

    let result = client
        .say_hello_with_delay(HelloWithDelayRequest {
            name: "Simon".into(),
            delay_in_seconds: 10,
        })
        .await
        .unwrap();

    assert_eq!(result.into_inner().message, "Hello Simon");
}

#[test(tokio::test)]
async fn execute_query() {
    let dummy = DummyServer {};

    let router = GrpcContext::default()
        .with_router(GrpcRouter::new(Server::builder()).add_service(GreeterServer::new(dummy)));
    let client_protocols = {
        let mut protocols = HashSet::new();

        protocols.insert(protocol_name!(GreeterServer::<DummyServer>::NAME));

        protocols
    };
    let mut client_swarm = Swarm::new_ephemeral(|_| grpc::Behaviour::new(GrpcContext::default()));
    let mut server_swarm = Swarm::new_ephemeral(|_| grpc::Behaviour::new(router));

    let (multiaddr, _) = server_swarm.listen().await;
    let server_peer_id = *server_swarm.local_peer_id();

    client_swarm
        .behaviour_mut()
        .add_address(&server_peer_id, multiaddr);

    let outbound_connection = client_swarm.behaviour_mut().open_outbound_connection(
        &server_peer_id,
        protocol_name!(GreeterServer::<DummyServer>::NAME),
    );

    let client_swarm = async move {
        loop {
            client_swarm.next_swarm_event().await;
        }
    };
    let server_swarm = async move {
        loop {
            server_swarm.next_swarm_event().await;
        }
    };

    spawn(server_swarm);
    spawn(client_swarm);
    let connection = outbound_connection.into_future().await.unwrap();

    let mut client = GreeterClient::new(connection.channel);

    let result = client
        .say_hello(HelloRequest {
            name: "Simon".into(),
        })
        .await
        .unwrap();

    assert_eq!(result.into_inner().message, "Hello Simon");
}

#[rstest]
fn create_context_with_only_router() {
    let context = GrpcContext::default().with_router(GrpcRouter::new(Server::builder()));

    let (router, (inbound, outbound)) = context.into_parts();

    assert!(router.is_some());
    assert_eq!(inbound, outbound);
}

#[rstest]
fn create_context_with_only_client() {
    let context = GrpcContext::default();

    let (router, (inbound, outbound)) = context.into_parts();

    assert!(router.is_none());
    assert_eq!(inbound, outbound);
}

#[rstest]
fn create_context_with_only_client_custom_protocol() {
    let context = GrpcContext::default().add_client_protocol("/custom");

    let (router, (inbound, outbound)) = context.into_parts();

    assert!(router.is_none());
    assert_ne!(inbound, outbound);
    assert_eq!(outbound.into_iter().collect::<Vec<_>>(), vec!["/custom"]);
    assert_eq!(
        inbound.into_iter().collect::<Vec<_>>(),
        Vec::<String>::new()
    );
}

#[test(tokio::test)]
async fn incompatible_protocol() {
    let dummy = DummyServer {};

    let router = GrpcContext::default()
        .with_router(GrpcRouter::new(Server::builder()).add_service(GreeterServer::new(dummy)));

    let client_protocols = {
        let mut protocols = HashSet::new();

        protocols.insert(protocol_name!(
            NoopServer::<implementations::NoopServer>::NAME
        ));

        protocols
    };

    let mut client_swarm = Swarm::new_ephemeral(|_| {
        grpc::Behaviour::new(GrpcContext::default().with_client_protocols(client_protocols))
    });
    let mut server_swarm = Swarm::new_ephemeral(|_| grpc::Behaviour::new(router));

    let server_peer_id = *server_swarm.local_peer_id();

    let (multiaddr, _) = server_swarm.listen().await;
    client_swarm
        .behaviour_mut()
        .add_address(&server_peer_id, multiaddr);

    let outbound_connection = client_swarm.behaviour_mut().open_outbound_connection(
        &server_peer_id,
        protocol_name!(NoopServer::<implementations::NoopServer>::NAME),
    );

    let client_swarm = async move {
        loop {
            client_swarm.next_swarm_event().await;
        }
    };
    let server_swarm = async move {
        loop {
            server_swarm.next_swarm_event().await;
        }
    };

    spawn(server_swarm);
    spawn(client_swarm);

    let result = outbound_connection.into_future().await;

    assert!(result.is_err());

    assert!(matches!(
        result,
        Err(OutboundConnectionError::Outbound(
            OutboundError::UnsupportedProtocol(_)
        ))
    ));
}
