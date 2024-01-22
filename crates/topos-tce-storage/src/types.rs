use topos_core::{
    api::grpc::checkpoints::SourceStreamPosition,
    types::{
        stream::{CertificateSourceStreamPosition, CertificateTargetStreamPosition, Position},
        CertificateDelivered, Ready, Signature,
    },
    uci::{Certificate, CertificateId},
};

use crate::{
    rocks::{db_column::DBColumn, TargetSourceListKey},
    CertificatePositions, PendingCertificateId,
};

pub type Echo = String;

pub type CertificateSequenceNumber = u64;
pub type EpochId = u64;
pub type Validators = Vec<String>;

/// Column that keeps certificates that are not yet delivered
pub(crate) type PendingCertificatesColumn = DBColumn<u64, Certificate>;
/// Column that keeps list of all certificates retrievable by their id
pub(crate) type CertificatesColumn = DBColumn<CertificateId, CertificateDelivered>;
/// Column that keeps list of certificates received from particular subnet and
/// maps (source subnet id, source certificate position) to certificate id
pub(crate) type StreamsColumn = DBColumn<CertificateSourceStreamPosition, CertificateId>;
/// Column that keeps list of certificates that are delivered to target subnet,
/// and maps their target (target subnet, source subnet and position/count per source subnet)
/// to certificate id
pub(crate) type TargetStreamsColumn = DBColumn<CertificateTargetStreamPosition, CertificateId>;
/// Keeps position for particular target subnet id <- source subnet id column in TargetStreamsColumn
pub(crate) type TargetSourceListColumn = DBColumn<TargetSourceListKey, Position>;

#[derive(Debug, Clone)]
pub enum PendingResult {
    AlreadyDelivered,
    AlreadyPending,
    AwaitPrecedence,
    InPending(PendingCertificateId),
}

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
#[allow(unused)]
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
