use std::future::IntoFuture;

use futures::{future::BoxFuture, FutureExt};
use tokio::sync::mpsc;

use crate::{client::StorageClient, command::StorageCommand, errors::StorageError, Storage};

pub struct Connection<S: Storage> {
    storage: S,
    queries: mpsc::Receiver<StorageCommand>,
}

impl<S: Storage> Connection<S> {
    pub fn new(storage: S) -> (Self, StorageClient) {
        let (sender, queries) = mpsc::channel(1024);

        (Self { storage, queries }, StorageClient { sender })
    }

    async fn handle_command(&mut self, command: StorageCommand) {
        match command {
            StorageCommand::Persist {
                certificate,
                status,
                response_channel,
            } => {
                if let Err(error) = self.storage.persist(certificate, status).await {
                    _ = response_channel.send(Err(error.into()));
                } else {
                    _ = response_channel.send(Ok(()));
                }
            }

            StorageCommand::UpdateCertificate {
                certificate_id,
                status,
                response_channel,
            } => {
                if let Err(error) = self.storage.update(&certificate_id, status).await {
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
                    self.storage
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
        async move {
            self.storage.connect().await?;

            loop {
                tokio::select! {
                    Some(command) = self.queries.recv() => {
                        self.handle_command(command).await;
                    }
                }
            }
        }
        .boxed()
    }
}
