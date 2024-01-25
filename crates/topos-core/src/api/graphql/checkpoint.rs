use async_graphql::InputObject;

use super::{certificate::CertificateId, subnet::SubnetId};

#[derive(InputObject)]
pub struct SourceStreamPositionInput {
    pub source_subnet_id: SubnetId,
    pub position: u64,
    pub certificate_id: Option<CertificateId>,
}

#[derive(InputObject)]
pub struct SourceCheckpointInput {
    pub source_subnet_ids: Vec<SubnetId>,
    pub positions: Vec<SourceStreamPositionInput>,
}
