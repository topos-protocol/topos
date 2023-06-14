use topos_core::api::grpc::checkpoints::TargetStreamPosition;
use topos_core::uci::Certificate;

#[derive(Debug)]
pub enum StreamCommand {
    PushCertificate {
        certificate: Certificate,
        positions: Vec<TargetStreamPosition>,
    },
}
