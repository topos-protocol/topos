use async_graphql::{InputObject, SimpleObject};
use serde::{Deserialize, Serialize};

use crate::types::stream::CertificateSourceStreamPosition;

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
}

impl From<&CertificateSourceStreamPosition> for SourceStreamPosition {
    fn from(value: &CertificateSourceStreamPosition) -> Self {
        Self {
            source_subnet_id: (&value.subnet_id).into(),
            position: *value.position,
        }
    }
}

#[derive(InputObject)]
pub struct SourceCheckpointInput {
    pub source_subnet_ids: Vec<SubnetId>,
    pub positions: Vec<SourceStreamPositionInput>,
}
