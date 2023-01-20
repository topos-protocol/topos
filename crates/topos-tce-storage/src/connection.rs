use std::{collections::VecDeque, future::IntoFuture, sync::Arc};

use futures::{future::BoxFuture, FutureExt, Stream, TryFutureExt};
use tokio::sync::mpsc;
use tokio_stream::wrappers::ReceiverStream;
use topos_commands::CommandHandler;

use crate::{
    client::StorageClient,
    command::StorageCommand,
    errors::{InternalStorageError, StorageError},
    events::StorageEvent,
    PendingCertificateId, Storage,
};

use self::builder::{ConnectionBuilder, StorageBuilder};

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
            StorageClient::new(sender),
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
            let certificate_dispatcher = self.certificate_dispatcher.take().ok_or(
                StorageError::InternalStorage(InternalStorageError::UnableToStartStorage)
            )?;

            loop {
                tokio::select! {
                    Ok(permit) = certificate_dispatcher.reserve(), if !self.pending_certificates.is_empty() => {

                        permit.send(self.pending_certificates.pop_front().unwrap());
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
                            FetchCertificates,
                            GetCertificate,
                            GetSourceHead,
                            RemovePendingCertificate)
                    }
                    else => break
                }
            }

            Ok(())
        }
        .boxed()
    }
}
