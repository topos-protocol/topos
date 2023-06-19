use async_graphql::{InputObject, SimpleObject};
use serde::{Deserialize, Serialize};

use super::subnet::SubnetId;

#[derive(Debug, Default, Serialize, Deserialize, InputObject)]
pub struct CertificateId {
    pub value: String,
}

#[derive(Debug, Default, Serialize, Deserialize, SimpleObject)]
#[serde(rename_all = "camelCase")]
pub struct Certificate {
    pub id: String,
    pub prev_id: String,
    pub proof: String,
    pub signature: String,
    pub source_subnet_id: String,
    pub state_root: String,
    pub target_subnets: Vec<SubnetId>,
    pub tx_root_hash: String,
    pub verifier: u32,
}

impl From<topos_uci::Certificate> for Certificate {
    fn from(uci_cert: topos_uci::Certificate) -> Self {
        Self {
            id: uci_cert.id.to_string(),
            prev_id: uci_cert.prev_id.to_string(),
            proof: hex::encode(&uci_cert.proof),
            signature: hex::encode(&uci_cert.signature),
            source_subnet_id: uci_cert.source_subnet_id.to_string(),
            state_root: hex::encode(uci_cert.state_root),
            target_subnets: uci_cert
                .target_subnets
                .into_iter()
                .map(SubnetId::from)
                .collect(),
            tx_root_hash: hex::encode(uci_cert.tx_root_hash),
            verifier: uci_cert.verifier,
        }
    }
}

impl From<topos_uci::SubnetId> for SubnetId {
    fn from(uci_id: topos_uci::SubnetId) -> Self {
        SubnetId {
            value: uci_id.to_string(),
        }
    }
}
