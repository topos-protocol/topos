use std::collections::HashMap;
use std::sync::Arc;

use async_graphql::{Context, EmptyMutation, Object, Schema, Subscription};
use async_trait::async_trait;
use futures::{Stream, StreamExt};
use tokio::sync::{mpsc, oneshot};
use topos_core::api::graphql::certificate::UndeliveredCertificate;
use topos_core::api::graphql::checkpoint::SourceStreamPosition;
use topos_core::api::graphql::errors::GraphQLServerError;
use topos_core::api::graphql::filter::SubnetFilter;
use topos_core::api::graphql::{
    certificate::{Certificate, CertificateId},
    checkpoint::SourceCheckpointInput,
    query::CertificateQuery,
};
use topos_core::types::stream::CertificateSourceStreamPosition;
use topos_metrics::{STORAGE_PENDING_POOL_COUNT, STORAGE_PRECEDENCE_POOL_COUNT};
use topos_tce_storage::fullnode::FullNodeStore;
use topos_tce_storage::store::ReadStore;

use topos_tce_storage::validator::ValidatorStore;
use tracing::debug;

use crate::runtime::InternalRuntimeCommand;
use crate::stream::TransientStream;

use super::filter::FilterIs;

pub struct QueryRoot;
pub(crate) type ServiceSchema = Schema<QueryRoot, EmptyMutation, SubscriptionRoot>;

#[async_trait]
impl CertificateQuery for QueryRoot {
    async fn certificates_per_subnet(
        ctx: &Context<'_>,
        from_source_checkpoint: SourceCheckpointInput,
        first: usize,
    ) -> Result<Vec<Certificate>, GraphQLServerError> {
        let store = ctx.data::<Arc<FullNodeStore>>().map_err(|_| {
            tracing::error!("Failed to get store from context");

            GraphQLServerError::ParseDataConnector
        })?;

        let mut certificates = Vec::default();

        for (index, _) in from_source_checkpoint.source_subnet_ids.iter().enumerate() {
            let subnet_id: topos_core::uci::SubnetId = (&from_source_checkpoint.positions[index]
                .source_subnet_id)
                .try_into()
                .map_err(|_| GraphQLServerError::ParseSubnetId)?;

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
                    .map(|(ref c, _)| c.into()),
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
                    .try_into()
                    .map_err(|_| GraphQLServerError::ParseCertificateId)?,
            )
            .map_err(|_| GraphQLServerError::StorageError)
            .and_then(|c| {
                c.map(|ref c| c.into())
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
        from_source_checkpoint: SourceCheckpointInput,
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

    /// This endpoint is used to get the current storage pool stats.
    /// It returns the number of certificates in the pending and precedence pools.
    /// The values are estimated as having a precise count is costly.
    async fn get_storage_pool_stats(
        &self,
        ctx: &Context<'_>,
    ) -> Result<HashMap<&str, i64>, GraphQLServerError> {
        let mut stats = HashMap::new();
        stats.insert("metrics_pending_pool", STORAGE_PENDING_POOL_COUNT.get());
        stats.insert(
            "metrics_precedence_pool",
            STORAGE_PRECEDENCE_POOL_COUNT.get(),
        );

        let store = ctx.data::<Arc<ValidatorStore>>().map_err(|_| {
            tracing::error!("Failed to get store from context");

            GraphQLServerError::ParseDataConnector
        })?;

        stats.insert(
            "count_pending_certificates",
            store
                .iter_pending_pool()
                .map_err(|_| GraphQLServerError::StorageError)?
                .count()
                .try_into()
                .unwrap_or(i64::MAX),
        );

        stats.insert(
            "count_precedence_certificates",
            store
                .iter_precedence_pool()
                .map_err(|_| GraphQLServerError::StorageError)?
                .count()
                .try_into()
                .unwrap_or(i64::MAX),
        );

        stats.insert(
            "pending_pool_size",
            store
                .pending_pool_size()
                .map_err(|_| GraphQLServerError::StorageError)?
                .try_into()
                .unwrap_or(i64::MAX),
        );

        stats.insert(
            "precedence_pool_size",
            store
                .precedence_pool_size()
                .map_err(|_| GraphQLServerError::StorageError)?
                .try_into()
                .unwrap_or(i64::MAX),
        );

        Ok(stats)
    }

    /// This endpoint is used to get the current checkpoint of the source streams.
    /// The checkpoint is the position of the last certificate delivered for each source stream.
    async fn get_checkpoint(
        &self,
        ctx: &Context<'_>,
    ) -> Result<Vec<SourceStreamPosition>, GraphQLServerError> {
        let store = ctx.data::<Arc<FullNodeStore>>().map_err(|_| {
            tracing::error!("Failed to get store from context");

            GraphQLServerError::ParseDataConnector
        })?;

        let checkpoint = store
            .get_checkpoint()
            .map_err(|_| GraphQLServerError::StorageError)?;

        Ok(checkpoint
            .iter()
            .map(|(subnet_id, head)| SourceStreamPosition {
                source_subnet_id: subnet_id.into(),
                position: *head.position,
                certificate_id: head.certificate_id.into(),
            })
            .collect())
    }

    /// This endpoint is used to get the current pending pool.
    /// It returns [`CertificateId`] and the [`PendingCertificateId`]
    async fn get_pending_pool(
        &self,
        ctx: &Context<'_>,
    ) -> Result<HashMap<u64, CertificateId>, GraphQLServerError> {
        let store = ctx.data::<Arc<ValidatorStore>>().map_err(|_| {
            tracing::error!("Failed to get store from context");

            GraphQLServerError::ParseDataConnector
        })?;

        Ok(store
            .iter_pending_pool()
            .map_err(|_| GraphQLServerError::StorageError)?
            .map(|(id, certificate)| (id, certificate.id.into()))
            .collect())
    }

    /// This endpoint is used to check if a certificate has any child certificate in the precedence pool.
    async fn check_precedence(
        &self,
        ctx: &Context<'_>,
        certificate_id: CertificateId,
    ) -> Result<Option<UndeliveredCertificate>, GraphQLServerError> {
        let store = ctx.data::<Arc<ValidatorStore>>().map_err(|_| {
            tracing::error!("Failed to get store from context");

            GraphQLServerError::ParseDataConnector
        })?;

        store
            .check_precedence(
                &certificate_id
                    .try_into()
                    .map_err(|_| GraphQLServerError::ParseCertificateId)?,
            )
            .map_err(|_| GraphQLServerError::StorageError)
            .map(|certificate| certificate.as_ref().map(Into::into))
    }
}

pub struct SubscriptionRoot;

impl SubscriptionRoot {
    /// Try to create a new [`Stream`] of delivered [`Certificate`]s to be used in a GraphQL subscription.
    pub(crate) async fn new_transient_stream(
        &self,
        register: &mpsc::Sender<InternalRuntimeCommand>,
        filter: Option<SubnetFilter>,
    ) -> Result<impl Stream<Item = Certificate>, GraphQLServerError> {
        let (sender, receiver) = oneshot::channel();
        _ = register
            .send(InternalRuntimeCommand::NewTransientStream { sender })
            .await;

        let stream: TransientStream = receiver
            .await
            .map_err(|_| {
                GraphQLServerError::InternalError(
                    "Communication error trying to create a new transient stream",
                )
            })?
            .map_err(|e| GraphQLServerError::TransientStream(e.to_string()))?;

        let filter: Option<(FilterIs, topos_core::uci::SubnetId)> = filter
            .map(|value| match value {
                SubnetFilter::Target(ref id) => id.try_into().map(|v| (FilterIs::Target, v)),
                SubnetFilter::Source(ref id) => id.try_into().map(|v| (FilterIs::Source, v)),
            })
            .map_or(Ok(None), |v| v.map(Some))
            .map_err(|_| GraphQLServerError::ParseSubnetId)?;

        Ok(stream
            .filter(move |c| {
                futures::future::ready(
                    filter
                        .as_ref()
                        .map(|v| match v {
                            (FilterIs::Source, id) => id == &c.certificate.source_subnet_id,
                            (FilterIs::Target, id) => c.certificate.target_subnets.contains(id),
                        })
                        .unwrap_or(true),
                )
            })
            .map(|c| c.as_ref().into()))
    }
}

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
        filter: Option<SubnetFilter>,
    ) -> Result<impl Stream<Item = Certificate>, GraphQLServerError> {
        let register = ctx
            .data::<mpsc::Sender<InternalRuntimeCommand>>()
            .map_err(|_| {
                tracing::error!("Failed to get the transient register client from context");

                GraphQLServerError::ParseDataConnector
            })?;

        self.new_transient_stream(register, filter).await
    }
}
