use topos_uci::{Certificate, CertificateId};

use serde::{Deserialize, Serialize};

use self::stream::{Position, SourceStreamPositionKey};
use topos_api::grpc::{
    checkpoints::SourceStreamPosition,
    tce::v1::{ProofOfDelivery as GrpcProofOfDelivery, SignedReady},
};

pub mod stream;

pub type Ready = String;
pub type Signature = String;

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
pub struct CertificateDelivered {
    pub certificate: Certificate,
    pub proof_of_delivery: ProofOfDelivery,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
pub struct ProofOfDelivery {
    pub certificate_id: CertificateId,
    pub delivery_position: SourceStreamPositionKey,
    pub readies: Vec<(Ready, Signature)>,
    pub threshold: u64,
}

impl From<SourceStreamPosition> for SourceStreamPositionKey {
    fn from(value: SourceStreamPosition) -> Self {
        Self(value.source_subnet_id, Position(value.position))
    }
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
