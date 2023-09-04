use async_graphql::{Context, EmptyMutation, EmptySubscription, Object, Schema};
use async_trait::async_trait;
use topos_api::graphql::errors::GraphQLServerError;
use topos_api::graphql::{
    certificate::{Certificate, CertificateId},
    checkpoint::SourceCheckpoint,
    query::CertificateQuery,
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
        let storage = ctx.data::<StorageClient>().map_err(|_| {
            tracing::error!("Failed to get storage client from context");

            GraphQLServerError::ParseDataConnector
        })?;

        let mut certificates = Vec::default();

        for (index, _) in from_source_checkpoint.source_subnet_ids.iter().enumerate() {
            let certificates_with_position = storage
                .fetch_certificates(FetchCertificatesFilter::Source {
                    source_stream_position: CertificateSourceStreamPosition {
                        source_subnet_id: topos_core::uci::SubnetId::try_from(
                            &from_source_checkpoint.positions[index].source_subnet_id,
                        )?,
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

    async fn certificate_by_id(
        ctx: &Context<'_>,
        certificate_id: CertificateId,
    ) -> Result<Certificate, GraphQLServerError> {
        let storage = ctx.data::<StorageClient>().map_err(|_| {
            tracing::error!("Failed to get storage client from context");

            GraphQLServerError::ParseDataConnector
        })?;

        storage
            .get_certificate(certificate_id.value.into())
            .await
            .map_err(|_| GraphQLServerError::StorageError)
            .map(|c| c.into())
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
