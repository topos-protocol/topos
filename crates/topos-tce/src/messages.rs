use serde::{Deserialize, Serialize};
use tce_transport::TceCommands;

/// Definition of networking payload.
///
/// We assume that only Commands will go through the network,
/// [Response] is used to allow reporting of logic errors to the caller.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[allow(clippy::large_enum_variant)]
pub enum NetworkMessage {
    Cmd(TceCommands),
    Bulk(Vec<TceCommands>),

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

// transformer of protocol commands into network commands
impl From<TceCommands> for NetworkMessage {
    fn from(cmd: TceCommands) -> Self {
        Self::Cmd(cmd)
    }
}
