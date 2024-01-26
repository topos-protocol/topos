use crate::api::graphql::certificate::{Certificate, CertificateId};
use crate::api::graphql::checkpoint::SourceCheckpointInput;
use crate::api::graphql::errors::GraphQLServerError;

use async_graphql::Context;
use async_trait::async_trait;

#[async_trait]
pub trait CertificateQuery {
    async fn certificates_per_subnet(
        ctx: &Context<'_>,
        from_source_checkpoint: SourceCheckpointInput,
        first: usize,
    ) -> Result<Vec<Certificate>, GraphQLServerError>;

    async fn certificate_by_id(
        ctx: &Context<'_>,
        certificate_id: CertificateId,
    ) -> Result<Certificate, GraphQLServerError>;
}
