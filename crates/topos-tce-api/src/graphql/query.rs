use std::sync::Arc;

use async_graphql::{Context, EmptyMutation, EmptySubscription, Object, Schema, Subscription};
use async_trait::async_trait;
use futures::{Stream, StreamExt};
use tokio::sync::{mpsc, oneshot};
use topos_api::graphql::errors::GraphQLServerError;
use topos_api::graphql::{
    certificate::{Certificate, CertificateId},
    checkpoint::SourceCheckpoint,
    query::CertificateQuery,
};
use topos_core::types::stream::CertificateSourceStreamPosition;
use topos_tce_storage::fullnode::FullNodeStore;
use topos_tce_storage::store::ReadStore;

use tracing::debug;

use crate::runtime::InternalRuntimeCommand;
use crate::stream::TransientStream;

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
            let position = from_source_checkpoint.positions[index].position.into();

            let certificates_with_position = store
                .get_source_stream_certificates_from_position(
                    CertificateSourceStreamPosition {
                        subnet_id,
                        position,
                    },
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

pub struct SubscriptionRoot;

#[Subscription]
impl SubscriptionRoot {
    /// This endpoint is used to received delivered certificates.
    /// It uses a transient stream, which is a stream that is only valid for the current connection.
    ///
    /// Closing the connection will close the stream.
    /// Starting a new connection will start a new stream and the client will not receive
    /// any certificates that were delivered before the connection was started.
    async fn watch_delivered_certificates(
        &self,
        ctx: &Context<'_>,
    ) -> Result<impl Stream<Item = Certificate>, GraphQLServerError> {
        let register = ctx
            .data::<mpsc::Sender<InternalRuntimeCommand>>()
            .map_err(|_| {
                tracing::error!("Failed to get the transient register client from context");

                GraphQLServerError::ParseDataConnector
            })?;

        let (sender, receiver) = oneshot::channel();
        _ = register
            .send(InternalRuntimeCommand::NewTransientStream { sender })
            .await;

        let stream: TransientStream = receiver.await.unwrap().unwrap();

        Ok(stream.map(|c| c.into()))
    }
}
