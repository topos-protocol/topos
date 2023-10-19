use std::iter;

use libp2p::{core::UpgradeInfo, InboundUpgrade, OutboundUpgrade, Stream};

use crate::constants::GRPC_P2P_TOPOS_PROTOCOL;

/// UpgradeProtocol for gRPC Connection
///
/// This protocol is used to upgrade the connection to a gRPC connection.
/// It is used by the `Handler` to upgrade the connection to a gRPC connection.
/// The gRPC protocol is defined as constant but can be updated to manage different
/// version of the protocol.
///
/// The `UpgradeInfo` trait is implemented to provide the protocol information.
/// The `OutboundUpgrade` and `InboundUpgrade` traits are implemented to provide
/// the upgrade of the connection. The upgrade is done by returning the socket
/// wrapped in a `Future`.
pub struct GrpcUpgradeProtocol {}

impl UpgradeInfo for GrpcUpgradeProtocol {
    type Info = String;

    type InfoIter = iter::Once<Self::Info>;

    fn protocol_info(&self) -> Self::InfoIter {
        iter::once(GRPC_P2P_TOPOS_PROTOCOL.into())
    }
}

impl OutboundUpgrade<Stream> for GrpcUpgradeProtocol {
    type Output = Stream;

    type Error = std::io::Error;

    type Future = futures::future::Ready<Result<Self::Output, Self::Error>>;

    fn upgrade_outbound(self, socket: Stream, _info: Self::Info) -> Self::Future {
        futures::future::ready(Ok(socket))
    }
}

impl InboundUpgrade<Stream> for GrpcUpgradeProtocol {
    type Output = Stream;

    type Error = std::io::Error;

    type Future = futures::future::Ready<Result<Self::Output, Self::Error>>;

    fn upgrade_inbound(self, socket: Stream, info: Self::Info) -> Self::Future {
        futures::future::ready(Ok(socket))
    }
}
