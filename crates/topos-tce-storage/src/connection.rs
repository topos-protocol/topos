use std::{collections::VecDeque, future::IntoFuture, pin::Pin, sync::Arc};

use async_trait::async_trait;
use futures::{future::BoxFuture, Future, FutureExt, Stream, TryFutureExt};
use tokio::sync::mpsc;
use tokio_stream::wrappers::ReceiverStream;
use topos_core::uci::Certificate;

use crate::{
    client::StorageClient,
    command::{
        AddPendingCertificate, CertificateDelivered, Command, GetCertificate, StorageCommand,
    },
    errors::{InternalStorageError, StorageError},
    PendingCertificateId, Storage, StorageEvent,
};

use self::builder::{ConnectionBuilder, StorageBuilder};

#[allow(dead_code)]
const MAX_PENDING_CERTIFICATES: usize = 1000;

mod builder;
mod handlers;

#[async_trait]
trait CommandHandler<C: Command> {
    async fn handle(&mut self, command: C) -> Result<C::Result, StorageError>;
}

pub struct Connection<S: Storage> {
    /// Manage the underlying storage
    storage: Arc<S>,

    /// Listen for queries from outside
    queries: mpsc::Receiver<StorageCommand>,

    /// Send storage events
    #[allow(dead_code)]
    events: mpsc::Sender<StorageEvent>,

    #[allow(dead_code)]
    certificate_dispatcher: Option<mpsc::Sender<PendingCertificateId>>,

    pending_certificates: VecDeque<PendingCertificateId>,
}

impl<S: Storage> Connection<S> {
    pub fn build(
        storage: StorageBuilder<S>,
    ) -> (
        ConnectionBuilder<S>,
        StorageClient,
        impl Stream<Item = StorageEvent>,
    ) {
        let (sender, queries) = mpsc::channel(1024);
        let (events, events_stream) = mpsc::channel(1024);
        let (certificate_dispatcher, _dispatchable_certificates) = mpsc::channel(10);

        (
            ConnectionBuilder {
                storage_builder: Some(storage),
                queries,
                events,
                certificate_dispatcher,
            },
            StorageClient { sender },
            ReceiverStream::new(events_stream),
        )
    }
}

impl<S: Storage> IntoFuture for ConnectionBuilder<S> {
    type Output = Result<(), StorageError>;

    type IntoFuture = BoxFuture<'static, Self::Output>;

    fn into_future(mut self) -> Self::IntoFuture {
        self.storage_builder
            .take()
            .unwrap_or_else(|| {
                futures::future::err(StorageError::InternalStorage(
                    InternalStorageError::UnableToStartStorage,
                ))
                .boxed()
            })
            .and_then(|storage| self.into_connection(storage).into_future())
            .boxed()
    }
}

impl<S: Storage> IntoFuture for Connection<S> {
    type Output = Result<(), StorageError>;

    type IntoFuture = BoxFuture<'static, Self::Output>;

    fn into_future(mut self) -> Self::IntoFuture {
        async move {
            let certificate_dispatcher = self.certificate_dispatcher.take().ok_or_else(|| {
                StorageError::InternalStorage(InternalStorageError::UnableToStartStorage)
            })?;

            loop {
                tokio::select! {
                    Ok(permit) = certificate_dispatcher.reserve(), if !self.pending_certificates.is_empty() => {

                        permit.send(self.pending_certificates.pop_front().unwrap());
                    },

                    Some(command) = self.queries.recv() => {
                        match command {
                            StorageCommand::AddPendingCertificate(command, response_channel) => {
                            let res = self.handle(command).await;
                                _ = response_channel.send(res);
                            },
                            StorageCommand::CertificateDelivered(command, response_channel) => {
                                _ = response_channel.send(self.handle(command).await)
                            },
                            _ => {}

                        }
                    }
                }
            }
        }
        .boxed()
    }
}
