use tokio::sync::oneshot;
use topos_core::uci::Certificate;
use topos_p2p::PeerId;

use super::error::RuntimeError;

pub enum RuntimeEvent {
    CertificateSubmitted {
        certificate: Certificate,
        sender: oneshot::Sender<Result<(), RuntimeError>>,
    },

    PeerListPushed {
        peers: Vec<PeerId>,
        sender: oneshot::Sender<Result<(), RuntimeError>>,
    },
}
