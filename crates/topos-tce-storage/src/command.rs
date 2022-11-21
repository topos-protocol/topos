use tokio::sync::{mpsc, oneshot};
use topos_core::uci::{Certificate, CertificateId};

use crate::{errors::StorageError, FetchCertificatesFilter, PendingCertificateId};

macro_rules! RegisterCommands {
    ($($command:ident),+) => {
        #[derive(Debug)]
        pub enum StorageCommand {
            $(
                $command(
                    $command,
                    oneshot::Sender<Result<<$command as Command>::Result, StorageError>>,
                ),
            )*
        }

        $(

            impl $command {
                #[allow(dead_code)]
                pub(crate) async fn send_to(self, tx: &mpsc::Sender<StorageCommand>) -> Result<<Self as Command>::Result, StorageError> {
                    let (response_channel, receiver) = oneshot::channel();

                    tx.send(StorageCommand::$command(self, response_channel)).await?;

                    receiver.await?
                }
            }
        )*
    };
}

// TODO: Replace by inventory
RegisterCommands!(
    AddPendingCertificate,
    CertificateDelivered,
    GetCertificate,
    FetchCertificates
);

pub trait Command {
    type Result: 'static;
}

#[derive(Debug)]
pub struct AddPendingCertificate {
    #[allow(dead_code)]
    pub(crate) certificate: Certificate,
}

impl Command for AddPendingCertificate {
    type Result = PendingCertificateId;
}

#[derive(Debug)]
pub struct CertificateDelivered {
    #[allow(dead_code)]
    pub(crate) certificate_id: CertificateId,
}

impl Command for CertificateDelivered {
    type Result = ();
}

#[derive(Debug)]
pub struct GetCertificate {
    #[allow(dead_code)]
    pub(crate) certificate_id: CertificateId,
}

impl Command for GetCertificate {
    type Result = Certificate;
}

#[derive(Debug)]
pub struct FetchCertificates {
    pub(crate) filter: FetchCertificatesFilter,
}

impl Command for FetchCertificates {
    type Result = Vec<Certificate>;
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use tokio::spawn;

    use super::*;

    #[tokio::test]
    async fn send_command() {
        let cert = Certificate::new("0".to_string(), "0".to_string(), vec![]);
        let command = AddPendingCertificate { certificate: cert };

        let (sender, mut receiver) = mpsc::channel(1);

        spawn(async move {
            tokio::time::timeout(Duration::from_micros(100), async move {
                match receiver.recv().await {
                    Some(StorageCommand::AddPendingCertificate(_, response_channel)) => {
                        _ = response_channel.send(Ok(1));
                    }
                    _ => unreachable!(),
                }
            })
            .await
        });

        assert!(command.send_to(&sender).await.is_ok());
    }
}
