use tokio::sync::oneshot;
use topos_core::uci::Certificate;

pub enum RuntimeEvent {
    CertificateSubmitted {
        certificate: Certificate,
        sender: oneshot::Sender<Result<(), ()>>,
    },
}

#[allow(dead_code)]
pub(crate) enum InternalRuntimeEvent {}
