use std::{future::IntoFuture, pin::Pin};

use futures::{future::BoxFuture, Future, FutureExt, TryFutureExt};
use tokio::sync::mpsc;

use crate::{client::StorageClient, command::StorageCommand, errors::StorageError, Storage};

pub type StorageBuilder<S> = Pin<Box<dyn Future<Output = Result<S, StorageError>> + Send>>;

pub struct Connection<S: Storage> {
    storage: StorageBuilder<S>,
    queries: mpsc::Receiver<StorageCommand>,
}

impl<S: Storage> Connection<S> {
    pub fn new(storage: StorageBuilder<S>) -> (Self, StorageClient) {
        let (sender, queries) = mpsc::channel(1024);

        (Self { storage, queries }, StorageClient { sender })
    }

    async fn handle_command(storage: &mut S, command: StorageCommand) {
        match command {
            StorageCommand::Persist {
                certificate,
                status,
                response_channel,
            } => match storage.persist(certificate, status).await {
                Err(error) => {
                    _ = response_channel.send(Err(error.into()));
                }
                Ok(value) => {
                    _ = response_channel.send(Ok(value));
                }
            },

            StorageCommand::UpdateCertificate {
                certificate_id,
                status,
                response_channel,
            } => {
                if let Err(error) = storage.update(&certificate_id, status).await {
                    _ = response_channel.send(Err(error.into()));
                } else {
                    _ = response_channel.send(Ok(()));
                }
            }

            StorageCommand::GetCertificate {
                certificate_id,
                response_channel,
            } => {
                _ = response_channel.send(
                    storage
                        .get_certificate(certificate_id)
                        .await
                        .map_err(Into::into),
                )
            }
            _ => {}
        }
    }
}

impl<S: Storage> IntoFuture for Connection<S> {
    type Output = Result<(), StorageError>;

    type IntoFuture = BoxFuture<'static, Self::Output>;

    fn into_future(mut self) -> Self::IntoFuture {
        self.storage
            .and_then(|mut storage| async move {
                loop {
                    tokio::select! {
                        Some(command) = self.queries.recv() => {
                            Self::handle_command(&mut storage, command).await;
                        }
                    }
                }
            })
            .boxed()
    }
}
