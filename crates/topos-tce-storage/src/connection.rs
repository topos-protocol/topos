use std::{future::IntoFuture, pin::Pin};

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

#[allow(dead_code)]
const MAX_PENDING_CERTIFICATES: usize = 1000;

pub type StorageBuilder<S> = Pin<Box<dyn Future<Output = Result<S, StorageError>> + Send>>;

pub struct ConnectionBuilder<S: Storage> {
    storage_builder: Option<StorageBuilder<S>>,

    queries: mpsc::Receiver<StorageCommand>,
    events: mpsc::Sender<StorageEvent>,
    certificate_dispatcher: mpsc::Sender<Certificate>,
}

pub struct Connection<S: Storage> {
    /// Manage the underlying storage
    storage: S,

    /// Listen for queries from outside
    queries: mpsc::Receiver<StorageCommand>,

    /// Send storage events
    #[allow(dead_code)]
    events: mpsc::Sender<StorageEvent>,

    #[allow(dead_code)]
    certificate_dispatcher: mpsc::Sender<Certificate>,
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
            loop {
                tokio::select! {

                    Some(command) = self.queries.recv() => {
                        match command {
                            StorageCommand::AddPendingCertificate(command, response_channel) => {
                                _ = response_channel.send(self.handle(command).await)
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

#[async_trait]
trait CommandHandler<C: Command> {
    async fn handle(&mut self, command: C) -> Result<C::Result, StorageError>;
}

#[async_trait]
impl<S> CommandHandler<AddPendingCertificate> for Connection<S>
where
    S: Storage,
{
    async fn handle(
        &mut self,
        AddPendingCertificate { certificate }: AddPendingCertificate,
    ) -> Result<PendingCertificateId, StorageError> {
        Ok(self.storage.add_pending_certificate(certificate).await?)
    }
}

#[async_trait]
impl<S> CommandHandler<CertificateDelivered> for Connection<S>
where
    S: Storage,
{
    async fn handle(&mut self, _command: CertificateDelivered) -> Result<(), StorageError> {
        Ok(())
    }
}

#[async_trait]
impl<S> CommandHandler<GetCertificate> for Connection<S>
where
    S: Storage,
{
    async fn handle(
        &mut self,
        GetCertificate { certificate_id }: GetCertificate,
    ) -> Result<Certificate, StorageError> {
        Ok(self.storage.get_certificate(certificate_id).await?)
    }
}

impl<S> ConnectionBuilder<S>
where
    S: Storage,
{
    fn into_connection(self, storage: S) -> Connection<S> {
        Connection {
            storage,
            queries: self.queries,
            events: self.events,
            certificate_dispatcher: self.certificate_dispatcher,
        }
    }
}
