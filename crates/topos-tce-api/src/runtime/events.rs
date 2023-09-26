use std::collections::HashMap;
use std::collections::HashSet;
use tokio::sync::oneshot;
use topos_core::uci::{Certificate, SubnetId};
use topos_p2p::PeerId;
use topos_tce_storage::types::PendingResult;

use super::error::RuntimeError;

pub enum RuntimeEvent {
    CertificateSubmitted {
        certificate: Box<Certificate>,
        sender: oneshot::Sender<Result<PendingResult, RuntimeError>>,
    },

    PeerListPushed {
        peers: Vec<PeerId>,
        sender: oneshot::Sender<Result<(), RuntimeError>>,
    },

    GetSourceHead {
        subnet_id: SubnetId,
        sender: oneshot::Sender<Result<Option<(u64, Certificate)>, RuntimeError>>,
    },

    GetLastPendingCertificates {
        subnet_ids: HashSet<SubnetId>,
        #[allow(clippy::type_complexity)]
        sender:
            oneshot::Sender<Result<HashMap<SubnetId, Option<(Certificate, u64)>>, RuntimeError>>,
    },
}
