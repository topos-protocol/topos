use std::{collections::VecDeque, future::IntoFuture, sync::Arc};

use futures::{future::BoxFuture, FutureExt, Stream, TryFutureExt};
use tokio::sync::{mpsc, oneshot};
use tokio_stream::wrappers::ReceiverStream;
use topos_commands::CommandHandler;
use tracing::{info, warn};

use crate::{
    client::StorageClient,
    command::StorageCommand,
    errors::{InternalStorageError, StorageError},
    events::StorageEvent,
    PendingCertificateId, Storage,
};

pub use self::builder::ConnectionBuilder;
use self::builder::StorageBuilder;

const MAX_PENDING_CERTIFICATES: usize = 1000;

mod builder;
mod handlers;

/// This struct is responsible of managing the Storage connection.
/// It consumes queries from outside and execute them on the storage.
pub struct Connection<S: Storage> {
    /// Manage the underlying storage
    storage: Arc<S>,

    /// Listen for queries from outside
    queries: mpsc::Receiver<StorageCommand>,

    /// Send storage events
    #[allow(dead_code)]
    events: mpsc::Sender<StorageEvent>,

    pending_certificates: VecDeque<PendingCertificateId>,

    /// Storage shutdown signal receiver
    pub(crate) shutdown: mpsc::Receiver<oneshot::Sender<()>>,
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
        let (shutdown_channel, shutdown_receiver) = mpsc::channel::<oneshot::Sender<()>>(1);

        (
            ConnectionBuilder {
                storage_builder: Some(storage),
                queries,
                events,
                shutdown_receiver,
            },
            StorageClient::new(sender, shutdown_channel),
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
        use crate::connection::StorageCommand::*;
        async move {
            let shutdowned: Option<oneshot::Sender<()>> = loop {
                tokio::select! {
                    shutdown = self.shutdown.recv() => {
                        break shutdown;
                    },
                    Some(command) = self.queries.recv() => {
                        macro_rules! HandleCommands {
                            ($($command_type:ident),+) => {
                                match command {
                                    $(
                                        $command_type(command, response_channel) => {
                                          _ = response_channel.send(self.handle(command).await)
                                        },
                                    )+
                                }
                            }
                        }
                        HandleCommands!(
                            AddPendingCertificate,
                            CertificateDelivered,
                            CheckPendingCertificateExists,
                            FetchCertificates,
                            GetCertificate,
                            GetPendingCertificates,
                            GetNextPendingCertificate,
                            GetSourceHead,
                            RemovePendingCertificate,
                            TargetedBy
                            )
                    }
                    else => break None
                }
            };

            if let Some(sender) = shutdowned {
                info!("Shutting down storage connection...");
                _ = sender.send(());
            } else {
                warn!("Shutting down storage connection due to error...");
            }

            Ok(())
        }
        .boxed()
    }
}
