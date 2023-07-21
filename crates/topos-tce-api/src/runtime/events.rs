use std::collections::HashMap;
use std::collections::HashSet;
use tokio::sync::oneshot;
use topos_core::uci::{Certificate, SubnetId};
use topos_p2p::PeerId;

use super::error::RuntimeError;

pub enum RuntimeEvent {
    CertificateSubmitted {
        certificate: Box<Certificate>,
        sender: oneshot::Sender<Result<(), RuntimeError>>,
    },

    PeerListPushed {
        peers: Vec<PeerId>,
        sender: oneshot::Sender<Result<(), RuntimeError>>,
    },

    GetSourceHead {
        subnet_id: SubnetId,
        sender: oneshot::Sender<Result<(u64, Certificate), RuntimeError>>,
    },

    GetLastPendingCertificates {
        subnet_ids: HashSet<SubnetId>,
        sender: oneshot::Sender<Result<HashMap<SubnetId, Option<Certificate>>, RuntimeError>>,
    },
}
