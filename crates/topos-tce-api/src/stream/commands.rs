use topos_core::api::grpc::checkpoints::TargetStreamPosition;
use topos_core::types::CertificateDelivered;

#[derive(Debug)]
pub enum StreamCommand {
    PushCertificate {
        certificate: CertificateDelivered,
        positions: Vec<TargetStreamPosition>,
    },
}
