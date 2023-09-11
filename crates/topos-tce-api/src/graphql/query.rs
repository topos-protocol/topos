use std::sync::Arc;

use async_graphql::{Context, EmptyMutation, EmptySubscription, Object, Schema};
use async_trait::async_trait;
use topos_api::graphql::errors::GraphQLServerError;
use topos_api::graphql::{
    certificate::{Certificate, CertificateId},
    checkpoint::SourceCheckpoint,
    query::CertificateQuery,
};
use topos_core::types::stream::{Position, SourceStreamPositionKey};
use topos_tce_storage::fullnode::FullNodeStore;
use topos_tce_storage::store::ReadStore;

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
        let store = ctx.data::<Arc<FullNodeStore>>().map_err(|_| {
            tracing::error!("Failed to get store from context");

            GraphQLServerError::ParseDataConnector
        })?;

        let mut certificates = Vec::default();

        for (index, _) in from_source_checkpoint.source_subnet_ids.iter().enumerate() {
            let subnet_id = topos_core::uci::SubnetId::try_from(
                &from_source_checkpoint.positions[index].source_subnet_id,
            )?;
            let position = Position(from_source_checkpoint.positions[index].position);

            let certificates_with_position = store
                .get_source_stream_certificates_from_position(
                    SourceStreamPositionKey(subnet_id, position),
                    first,
                )
                .map_err(|_| GraphQLServerError::StorageError)?;

            debug!("Returned from storage: {certificates_with_position:?}");
            certificates.extend(
                certificates_with_position
                    .into_iter()
                    .map(|(c, _)| c.certificate.into()),
            );
        }

        Ok(certificates)
    }

    async fn certificate_by_id(
        ctx: &Context<'_>,
        certificate_id: CertificateId,
    ) -> Result<Certificate, GraphQLServerError> {
        let store = ctx.data::<Arc<FullNodeStore>>().map_err(|_| {
            tracing::error!("Failed to get storage client from context");

            GraphQLServerError::ParseDataConnector
        })?;

        store
            .get_certificate(
                &certificate_id
                    .value
                    .as_bytes()
                    .try_into()
                    .map_err(|_| GraphQLServerError::ParseCertificateId)?,
            )
            .map_err(|_| GraphQLServerError::StorageError)
            .and_then(|c| {
                c.map(|c| c.certificate.into())
                    .ok_or(GraphQLServerError::StorageError)
            })
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

    async fn certificate(
        &self,
        ctx: &Context<'_>,
        certificate_id: CertificateId,
    ) -> Result<Certificate, GraphQLServerError> {
        Self::certificate_by_id(ctx, certificate_id).await
    }
}
