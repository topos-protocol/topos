use tokio::sync::oneshot;
use topos_core::uci::Certificate;

use super::error::RuntimeError;

pub enum RuntimeEvent {
    CertificateSubmitted {
        certificate: Certificate,
        sender: oneshot::Sender<Result<(), RuntimeError>>,
    },
}

#[allow(dead_code)]
pub(crate) enum InternalRuntimeEvent {}
