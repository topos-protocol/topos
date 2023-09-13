use serde::{Deserialize, Serialize};
use topos_core::{
    api::grpc::{
        checkpoints::SourceStreamPosition,
        tce::v1::{ProofOfDelivery as GrpcProofOfDelivery, SignedReady},
    },
    uci::{Certificate, CertificateId},
};

pub use crate::rocks::types::SourceStreamPositionKey;
use crate::{CertificatePositions, Position};

pub type Echo = String;
pub type Ready = String;
pub type Signature = String;
pub type CertificateSequenceNumber = u64;
pub type EpochId = u64;
pub type Participants = Vec<String>;

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
pub struct CertificateDelivered {
    pub certificate: Certificate,
    // TODO: Remove option when implement ProofOfDelivery on api
    pub proof_of_delivery: ProofOfDelivery,
}

#[derive(Debug, Clone)]
pub struct CertificateDeliveredWithPositions(pub CertificateDelivered, pub CertificatePositions);

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
pub struct ProofOfDelivery {
    pub certificate_id: CertificateId,
    pub delivery_position: SourceStreamPositionKey,
    pub readies: Vec<(Ready, Signature)>,
    pub threshold: u64,
}

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
pub struct VerifiedCheckpointSummary(CheckpointSummary, AuthorityQuorumSignatureInfo);

#[allow(unused)]
pub struct AuthorityQuorumSignatureInfo {
    epoch: EpochId,
    signature: [u8; 32],
}

#[allow(unused)]
pub struct BroadcastState {
    echoes: Vec<(Echo, Signature)>,
    readies: Vec<(Ready, Signature)>,
    delivered: bool,
}

impl From<GrpcProofOfDelivery> for ProofOfDelivery {
    fn from(value: GrpcProofOfDelivery) -> Self {
        let position: SourceStreamPosition = value.delivery_position.unwrap().try_into().unwrap();
        Self {
            certificate_id: position.certificate_id.unwrap(),
            delivery_position: position.into(),
            readies: value
                .readies
                .into_iter()
                .map(|v| (v.ready, v.signature))
                .collect(),
            threshold: value.threshold,
        }
    }
}
impl From<ProofOfDelivery> for GrpcProofOfDelivery {
    fn from(value: ProofOfDelivery) -> Self {
        Self {
            delivery_position: Some(
                SourceStreamPosition {
                    source_subnet_id: value.delivery_position.0,
                    position: value.delivery_position.1 .0,
                    certificate_id: Some(value.certificate_id),
                }
                .into(),
            ),
            readies: value
                .readies
                .into_iter()
                .map(|v| SignedReady {
                    ready: v.0,
                    signature: v.1,
                })
                .collect(),
            threshold: value.threshold,
        }
    }
}

impl From<SourceStreamPosition> for SourceStreamPositionKey {
    fn from(value: SourceStreamPosition) -> Self {
        Self(value.source_subnet_id, Position(value.position))
    }
}
