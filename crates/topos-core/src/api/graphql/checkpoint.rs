use async_graphql::InputObject;

use super::{certificate::CertificateId, subnet::SubnetId};

#[derive(InputObject)]
pub struct SourceStreamPosition {
    pub source_subnet_id: SubnetId,
    pub position: u64,
    pub certificate_id: Option<CertificateId>,
}

#[derive(InputObject)]
pub struct SourceCheckpoint {
    pub source_subnet_ids: Vec<SubnetId>,
    pub positions: Vec<SourceStreamPosition>,
}
