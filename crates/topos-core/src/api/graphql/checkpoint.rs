use async_graphql::InputObject;
use serde::{Deserialize, Serialize};

use super::{certificate::CertificateId, subnet::SubnetId};

#[derive(Debug, Default, Serialize, Deserialize, InputObject)]
pub struct SourceStreamPosition {
    pub source_subnet_id: SubnetId,
    pub position: u64,
    pub certificate_id: Option<CertificateId>,
}

#[derive(Debug, Default, Serialize, Deserialize, InputObject)]
pub struct SourceCheckpoint {
    pub source_subnet_ids: Vec<SubnetId>,
    pub positions: Vec<SourceStreamPosition>,
}
