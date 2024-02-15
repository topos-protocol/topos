use topos_core::{
    types::ValidatorId,
    uci::{Certificate, CertificateId},
};
use topos_crypto::messages::Signature;

/// Protocol events
#[derive(Clone, Debug)]
pub enum ProtocolEvents {
    BroadcastFailed {
        certificate_id: CertificateId,
    },
    AlreadyDelivered {
        certificate_id: CertificateId,
    },

    /// (pb.Broadcast)
    Broadcast {
        certificate_id: CertificateId,
    },
    /// Indicates that 'gossip' message broadcasting is required
    Gossip {
        cert: Certificate,
    },
    /// Indicates that 'echo' message broadcasting is required
    Echo {
        certificate_id: CertificateId,
        signature: Signature,
        validator_id: ValidatorId,
    },
    /// Indicates that 'ready' message broadcasting is required
    Ready {
        certificate_id: CertificateId,
        signature: Signature,
        validator_id: ValidatorId,
    },
}
