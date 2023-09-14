use topos_uci::{Certificate, CertificateId};

use serde::{Deserialize, Serialize};

use crate::errors::GrpcParsingError;

use self::stream::{CertificateSourceStreamPosition, Position};
use topos_api::grpc::{
    checkpoints::SourceStreamPosition,
    tce::v1::{ProofOfDelivery as GrpcProofOfDelivery, SignedReady},
    ConversionError,
};

pub mod stream;

pub type Ready = String;
pub type Signature = String;

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
pub struct CertificateDelivered {
    pub certificate: Certificate,
    pub proof_of_delivery: ProofOfDelivery,
}

/// Certificate's Proof of Delivery
///
/// This structure is used to prove that a certificate has been delivered.
/// It contains the certificate's ID, the position of the certificate in the
/// source stream, the list of Ready messages received and the threshold.
/// The threshold is the number of Ready messages required to consider the
/// certificate as delivered. For a certificate, multiple Proofs of Delivery
/// can be created on the network, each one with a different list of Ready messages.
///
/// Two different Proofs of Delivery for the same Certificate can still be valid
/// if their Ready messages are valid. Because of the threshold, a certificate
/// can be considered as delivered even with a different set of Ready messages,
/// it simply means that the node received a different set of Ready messages
/// than the other nodes.
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
pub struct ProofOfDelivery {
    /// The certificate's ID
    pub certificate_id: CertificateId,
    /// The position of the certificate in the source stream
    pub delivery_position: CertificateSourceStreamPosition,
    /// The list of Ready messages used to proove the certificate's delivery
    pub readies: Vec<(Ready, Signature)>,
    /// The threshold of Ready messages required to consider the certificate as delivered
    pub threshold: u64,
}

impl From<SourceStreamPosition> for CertificateSourceStreamPosition {
    fn from(value: SourceStreamPosition) -> Self {
        Self {
            subnet_id: value.source_subnet_id,
            position: value.position.into(),
        }
    }
}

impl TryFrom<GrpcProofOfDelivery> for ProofOfDelivery {
    type Error = GrpcParsingError;
    fn try_from(value: GrpcProofOfDelivery) -> Result<Self, Self::Error> {
        let position: SourceStreamPosition = value
            .delivery_position
            .ok_or(GrpcParsingError::GrpcMalformedType("position"))?
            .try_into()?;

        Ok(Self {
            certificate_id: position
                .certificate_id
                .ok_or(GrpcParsingError::GrpcMalformedType(
                    "position.certificate_id",
                ))?,
            delivery_position: position.into(),
            readies: value
                .readies
                .into_iter()
                .map(|v| (v.ready, v.signature))
                .collect(),
            threshold: value.threshold,
        })
    }
}

impl From<ProofOfDelivery> for GrpcProofOfDelivery {
    fn from(value: ProofOfDelivery) -> Self {
        Self {
            delivery_position: Some(
                SourceStreamPosition {
                    source_subnet_id: value.delivery_position.subnet_id,
                    position: *value.delivery_position.position,
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
