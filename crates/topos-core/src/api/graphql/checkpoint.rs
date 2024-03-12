use async_graphql::{InputObject, SimpleObject};
use serde::{Deserialize, Serialize};

use crate::types::ProofOfDelivery;

use super::{certificate::CertificateId, subnet::SubnetId};

#[derive(InputObject)]
pub struct SourceStreamPositionInput {
    pub source_subnet_id: SubnetId,
    pub position: u64,
    pub certificate_id: Option<CertificateId>,
}

#[derive(Debug, Deserialize, Serialize, SimpleObject)]
#[serde(rename_all = "camelCase")]
pub struct SourceStreamPosition {
    pub source_subnet_id: SubnetId,
    pub position: u64,
    pub certificate_id: CertificateId,
}

impl From<&ProofOfDelivery> for SourceStreamPosition {
    fn from(value: &ProofOfDelivery) -> Self {
        Self {
            certificate_id: value.certificate_id.into(),
            source_subnet_id: (&value.delivery_position.subnet_id).into(),
            position: *value.delivery_position.position,
        }
    }
}

#[derive(InputObject)]
pub struct SourceCheckpointInput {
    pub source_subnet_ids: Vec<SubnetId>,
    pub positions: Vec<SourceStreamPositionInput>,
}
