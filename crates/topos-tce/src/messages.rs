use serde::{Deserialize, Serialize};
use topos_core::api::grpc::tce::v1::{CheckpointRequest, DoubleEchoRequest};

/// Definition of networking payload.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[allow(clippy::large_enum_variant)]
pub enum NetworkMessage {
    Cmd(DoubleEchoRequest),
    Sync(CheckpointRequest),

    NotReady(topos_p2p::NotReadyMessage),
}

// deserializer
impl From<Vec<u8>> for NetworkMessage {
    fn from(data: Vec<u8>) -> Self {
        bincode::deserialize::<NetworkMessage>(data.as_ref()).expect("msg deser")
    }
}

// serializer
impl From<NetworkMessage> for Vec<u8> {
    fn from(msg: NetworkMessage) -> Self {
        bincode::serialize::<NetworkMessage>(&msg).expect("msg ser")
    }
}

// Transformer of double echo requests into network commands
impl From<DoubleEchoRequest> for NetworkMessage {
    fn from(request: DoubleEchoRequest) -> Self {
        Self::Cmd(request)
    }
}
