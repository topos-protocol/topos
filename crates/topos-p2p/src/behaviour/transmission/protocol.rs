use libp2p::core::ProtocolName;

use crate::constant::TRANSMISSION_PROTOCOL;

#[derive(Debug, Clone)]
pub(crate) struct TransmissionProtocol();

impl ProtocolName for TransmissionProtocol {
    fn protocol_name(&self) -> &[u8] {
        TRANSMISSION_PROTOCOL.as_bytes()
    }
}
