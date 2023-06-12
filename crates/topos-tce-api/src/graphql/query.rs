use async_graphql::{Context, EmptyMutation, EmptySubscription, Object, Schema};
use async_trait::async_trait;
use topos_api::graphql::errors::GraphQLServerError;
use topos_api::graphql::{
    certificate::Certificate, checkpoint::SourceCheckpoint, query::CertificateQuery,
};
use topos_tce_storage::{CertificateSourceStreamPosition, FetchCertificatesFilter, StorageClient};

use tracing::debug;

pub struct QueryRoot;
pub(crate) type ServiceSchema = Schema<QueryRoot, EmptyMutation, EmptySubscription>;

#[async_trait]
impl CertificateQuery for QueryRoot {
    async fn certificates_per_subnet(
        ctx: &Context<'_>,
        from_source_checkpoint: SourceCheckpoint,
        first: usize,
    ) -> Result<Vec<Certificate>, GraphQLServerError> {
        let storage = ctx
            .data::<StorageClient>()
            .map_err(|_| GraphQLServerError::ParseDataConnector)?;

        let mut certificates = Vec::default();

        for (index, _) in from_source_checkpoint.source_subnet_ids.iter().enumerate() {
            let certificates_with_position = storage
                .fetch_certificates(FetchCertificatesFilter::Source {
                    source_stream_position: CertificateSourceStreamPosition {
                        source_subnet_id: from_source_checkpoint.positions[index]
                            .source_subnet_id
                            .clone()
                            .to_uci_subnet_id()?,
                        position: topos_tce_storage::Position(
                            from_source_checkpoint.positions[index].position,
                        ),
                    },
                    limit: first,
                })
                .await
                .map_err(|_| GraphQLServerError::StorageError)?;
            debug!("Returned from storage: {certificates_with_position:?}");
            certificates.extend(
                certificates_with_position
                    .into_iter()
                    .map(|(c, _)| c.into()),
            );
        }

        Ok(certificates)
    }
}

#[Object]
impl QueryRoot {
    /// The endpoint for the GraphQL API, calling our trait implementation on the QueryRoot object
    async fn certificates(
        &self,
        ctx: &Context<'_>,
        from_source_checkpoint: SourceCheckpoint,
        first: usize,
    ) -> Result<Vec<Certificate>, GraphQLServerError> {
        Self::certificates_per_subnet(ctx, from_source_checkpoint, first).await
    }
}
