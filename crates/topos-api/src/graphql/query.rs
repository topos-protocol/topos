use crate::graphql::certificate::Certificate;
use crate::graphql::checkpoint::SourceCheckpoint;
use crate::graphql::errors::GraphQLServerError;

use async_graphql::Context;
use async_trait::async_trait;

#[async_trait]
pub trait CertificateQuery {
    async fn certificates_per_subnet(
        ctx: &Context<'_>,
        from_source_checkpoint: SourceCheckpoint,
        first: usize,
    ) -> Result<Vec<Certificate>, GraphQLServerError>;
}
