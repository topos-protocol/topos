use topos_core::{
    api::grpc::checkpoints::SourceStreamPosition,
    types::{CertificateDelivered, Ready, Signature},
};

use crate::CertificatePositions;

pub type Echo = String;

pub type CertificateSequenceNumber = u64;
pub type EpochId = u64;
pub type Validators = Vec<String>;

#[derive(Debug, Clone)]
pub struct CertificateDeliveredWithPositions(pub CertificateDelivered, pub CertificatePositions);

#[allow(unused)]
pub struct EpochSummary {
    epoch_id: EpochId,
    start_checkpoint: VerifiedCheckpointSummary,
    end_checkpoint: Option<VerifiedCheckpointSummary>,
}

#[allow(unused)]
pub struct CheckpointSummary {
    epoch: EpochId,
    sequence_number: usize,
    checkpoint_data: Vec<SourceStreamPosition>,
}
pub struct VerifiedCheckpointSummary(CheckpointSummary, ValidatorQuorumSignatureInfo);

#[allow(unused)]
pub struct ValidatorQuorumSignatureInfo {
    epoch: EpochId,
    signature: [u8; 32],
}

#[allow(unused)]
pub struct BroadcastState {
    echoes: Vec<(Echo, Signature)>,
    readies: Vec<(Ready, Signature)>,
    delivered: bool,
}
