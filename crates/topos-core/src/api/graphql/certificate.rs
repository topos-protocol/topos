use async_graphql::{NewType, SimpleObject};
use serde::{Deserialize, Serialize};

use crate::{types::CertificateDelivered, uci};

use super::{checkpoint::SourceStreamPosition, subnet::SubnetId};

#[derive(Serialize, Deserialize, Debug, NewType)]
pub struct CertificateId(String);

impl From<uci::CertificateId> for CertificateId {
    fn from(value: uci::CertificateId) -> Self {
        Self(value.to_string())
    }
}

#[derive(Serialize, Deserialize, Debug, SimpleObject)]
#[serde(rename_all = "camelCase")]
pub struct CertificatePositions {
    source: SourceStreamPosition,
}

#[derive(Debug, Serialize, Deserialize, SimpleObject)]
#[serde(rename_all = "camelCase")]
pub struct Certificate {
    pub id: CertificateId,
    pub prev_id: CertificateId,
    pub proof: String,
    pub signature: String,
    pub source_subnet_id: SubnetId,
    pub state_root: String,
    pub target_subnets: Vec<SubnetId>,
    pub tx_root_hash: String,
    pub receipts_root_hash: String,
    pub verifier: u32,
    pub positions: CertificatePositions,
}

#[derive(Debug, Serialize, Deserialize, SimpleObject)]
#[serde(rename_all = "camelCase")]
pub struct UndeliveredCertificate {
    pub id: CertificateId,
    pub prev_id: CertificateId,
    pub proof: String,
    pub signature: String,
    pub source_subnet_id: SubnetId,
    pub state_root: String,
    pub target_subnets: Vec<SubnetId>,
    pub tx_root_hash: String,
    pub receipts_root_hash: String,
    pub verifier: u32,
}

impl From<&crate::uci::Certificate> for UndeliveredCertificate {
    fn from(value: &crate::uci::Certificate) -> Self {
        Self {
            id: CertificateId(value.id.to_string()),
            prev_id: CertificateId(value.prev_id.to_string()),
            proof: hex::encode(&value.proof),
            signature: hex::encode(&value.signature),
            source_subnet_id: (&value.source_subnet_id).into(),
            state_root: hex::encode(value.state_root),
            target_subnets: value.target_subnets.iter().map(Into::into).collect(),
            tx_root_hash: hex::encode(value.tx_root_hash),
            receipts_root_hash: format!("0x{}", hex::encode(value.receipts_root_hash)),
            verifier: value.verifier,
        }
    }
}

#[derive(Debug, Serialize, Deserialize, SimpleObject)]
pub struct Ready {
    message: String,
    signature: String,
}

impl From<&CertificateDelivered> for Certificate {
    fn from(value: &CertificateDelivered) -> Self {
        let uci_cert = &value.certificate;

        Self {
            id: CertificateId(uci_cert.id.to_string()),
            prev_id: CertificateId(uci_cert.prev_id.to_string()),
            proof: hex::encode(&uci_cert.proof),
            signature: hex::encode(&uci_cert.signature),
            source_subnet_id: (&uci_cert.source_subnet_id).into(),
            state_root: hex::encode(uci_cert.state_root),
            target_subnets: uci_cert.target_subnets.iter().map(Into::into).collect(),
            tx_root_hash: hex::encode(uci_cert.tx_root_hash),
            receipts_root_hash: format!("0x{}", hex::encode(uci_cert.receipts_root_hash)),
            verifier: uci_cert.verifier,
            positions: CertificatePositions {
                source: (&value.proof_of_delivery).into(),
            },
        }
    }
}

impl TryFrom<CertificateId> for crate::uci::CertificateId {
    type Error = uci::Error;

    fn try_from(value: CertificateId) -> Result<Self, Self::Error> {
        crate::uci::CertificateId::try_from(value.0.as_bytes())
    }
}
