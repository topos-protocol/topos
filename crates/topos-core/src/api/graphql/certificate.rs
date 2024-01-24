use async_graphql::{NewType, SimpleObject};
use serde::{Deserialize, Serialize};

use crate::uci;

use super::subnet::SubnetId;

#[derive(Serialize, Deserialize, Debug, NewType)]
pub struct CertificateId(String);

#[derive(Serialize, Deserialize, Debug, SimpleObject)]
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
}

impl From<&crate::uci::Certificate> for Certificate {
    fn from(uci_cert: &crate::uci::Certificate) -> Self {
        Self {
            id: CertificateId(uci_cert.id.to_string()),
            prev_id: CertificateId(uci_cert.prev_id.to_string()),
            proof: hex::encode(&uci_cert.proof),
            signature: hex::encode(&uci_cert.signature),
            source_subnet_id: SubnetId::from(&uci_cert.source_subnet_id),
            state_root: hex::encode(uci_cert.state_root),
            target_subnets: uci_cert.target_subnets.iter().map(SubnetId::from).collect(),
            tx_root_hash: hex::encode(uci_cert.tx_root_hash),
            receipts_root_hash: format!("0x{}", hex::encode(uci_cert.receipts_root_hash)),
            verifier: uci_cert.verifier,
        }
    }
}

impl TryFrom<CertificateId> for crate::uci::CertificateId {
    type Error = uci::Error;

    fn try_from(value: CertificateId) -> Result<Self, Self::Error> {
        crate::uci::CertificateId::try_from(value.0.as_bytes())
    }
}
