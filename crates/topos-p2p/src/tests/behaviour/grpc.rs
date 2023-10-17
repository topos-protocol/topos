use std::{future::IntoFuture, time::Duration};

use async_trait::async_trait;
use libp2p::Swarm;
use libp2p_swarm_test::SwarmExt;
use rstest::rstest;
use test_log::test;
use tokio::spawn;
use tokio_util::sync::CancellationToken;
use tonic::{transport::Server, Request, Response, Status};
use topos_test_sdk::grpc::behaviour::helloworld::{
    greeter_client::GreeterClient,
    greeter_server::{Greeter, GreeterServer},
    HelloReply, HelloRequest, HelloWithDelayRequest,
};

use crate::behaviour::grpc;

#[derive(Default)]
struct DummyServer {}

#[async_trait]
impl Greeter for DummyServer {
    async fn say_hello(
        &self,
        request: Request<HelloRequest>,
    ) -> Result<Response<HelloReply>, Status> {
        Ok(Response::new(HelloReply {
            message: format!("Hello {}", request.into_inner().name),
        }))
    }

    async fn say_hello_with_delay(
        &self,
        request: Request<HelloWithDelayRequest>,
    ) -> Result<Response<HelloReply>, Status> {
        let request = request.into_inner();
        tokio::time::sleep(Duration::from_secs(request.delay_in_seconds)).await;

        Ok(Response::new(HelloReply {
            message: format!("Hello {}", request.name),
        }))
    }
}

#[rstest]
#[test(tokio::test)]
#[timeout(Duration::from_secs(5))]
async fn instantiate_grpc() {
    let dummy = DummyServer {};
    let router = Server::builder().add_service(GreeterServer::new(dummy));
    let mut client_swarm = Swarm::new_ephemeral(|_| grpc::Behaviour::new(None));
    let mut server_swarm = Swarm::new_ephemeral(|_| grpc::Behaviour::new(Some(router)));

    let server_peer_id = *server_swarm.local_peer_id();

    server_swarm.listen().await;

    let server_address = server_swarm.listeners().next().unwrap();

    client_swarm
        .behaviour_mut()
        .add_address(&server_peer_id, server_address.clone());

    let outbound_connection = client_swarm
        .behaviour_mut()
        .open_outbound_connection(&server_peer_id);

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
    let connection = outbound_connection.into_future().await.unwrap();

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
    let router = Server::builder().add_service(GreeterServer::new(dummy));
    let mut client_swarm = Swarm::new_ephemeral(|_| grpc::Behaviour::new(None));
    let mut server_swarm = Swarm::new_ephemeral(|_| grpc::Behaviour::new(Some(router)));

    let server_peer_id = *server_swarm.local_peer_id();

    server_swarm.listen().await;

    let server_address = server_swarm.listeners().next().unwrap();
    client_swarm
        .behaviour_mut()
        .add_address(&server_peer_id, server_address.clone());

    let outbound_connection = client_swarm
        .behaviour_mut()
        .open_outbound_connection(&server_peer_id);

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
    let router = Server::builder().add_service(GreeterServer::new(dummy));
    let mut client_swarm = Swarm::new_ephemeral(|_| grpc::Behaviour::new(None));
    let mut server_swarm = Swarm::new_ephemeral(|_| grpc::Behaviour::new(Some(router)));

    let server_peer_id = *server_swarm.local_peer_id();

    server_swarm.listen().await;
    client_swarm.connect(&mut server_swarm).await;
    let outbound_connection = client_swarm
        .behaviour_mut()
        .open_outbound_connection(&server_peer_id);

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
