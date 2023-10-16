use std::future::IntoFuture;

use futures::{future::BoxFuture, FutureExt};
use libp2p::{swarm::ConnectionId, Multiaddr};
use tokio::sync::oneshot;
use tonic::transport::Channel;

use super::{
    error::{OutboundConnectionError, OutboundError},
    RequestId,
};

/// Connection struct which represent a connection between two nodes
/// It contains the connection id, the address of the node, the request id
/// and the gRPC channel which is used to communicate with the node.
#[derive(Debug)]
pub(crate) struct Connection {
    /// The connection id
    pub(crate) id: ConnectionId,

    /// The address of the node
    pub(crate) address: Option<Multiaddr>,

    /// The request id that is served by this connection
    pub(crate) request_id: Option<RequestId>,

    /// The gRPC channel used to communicate with the node
    pub(crate) channel: Option<Channel>,
}

/// Connection request struct which is used to open a connection to a node
pub(crate) struct OutboundConnectionRequest {
    pub(crate) request_id: RequestId,
    pub(crate) notifier: oneshot::Sender<Result<Channel, OutboundError>>,
    pub(crate) protocol: String,
}

/// Struct which is used to represent a connected channel connection
#[derive(Debug)]
pub struct OutboundConnectedConnection {
    #[allow(dead_code)]
    pub(crate) request_id: RequestId,
    // TODO: Remove unused when gRPC behaviour is activated
    #[allow(unused)]
    pub(crate) channel: tonic::transport::Channel,
}

/// Enum that represents the different states of an outbound connection
#[derive(Debug)]
pub enum OutboundConnection {
    Connected(OutboundConnectedConnection),
    Pending {
        request_id: RequestId,
    },
    Opening {
        request_id: RequestId,
        receiver: oneshot::Receiver<Result<Channel, OutboundError>>,
    },
}

impl IntoFuture for OutboundConnection {
    type Output = Result<OutboundConnectedConnection, OutboundConnectionError>;

    type IntoFuture = BoxFuture<'static, Self::Output>;

    fn into_future(self) -> Self::IntoFuture {
        println!("Outbound future is in: {:?}", self);
        async move {
            match self {
                // The outbound connection is already opened
                OutboundConnection::Connected(connected) => Ok(connected),
                // The outbound connection is in pending
                OutboundConnection::Pending { request_id } => {
                    Err(OutboundConnectionError::AlreadyNegotiating)
                }
                // The connection is in opening state so we need to proceed to connect
                OutboundConnection::Opening {
                    request_id,
                    receiver,
                } => {
                    let channel = receiver.await??;

                    Ok(OutboundConnectedConnection {
                        request_id,
                        channel,
                    })
                }
            }
        }
        .boxed()
    }
}
