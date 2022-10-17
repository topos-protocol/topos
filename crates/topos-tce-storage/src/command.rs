use tokio::sync::oneshot;
use topos_core::uci::{Certificate, CertificateId};

use crate::{errors::StorageError, PendingCertificateId};

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
            impl From<$command> for (StorageCommand, oneshot::Receiver<Result<<$command as Command>::Result, StorageError>>) {
                fn from(command: $command) -> (StorageCommand, oneshot::Receiver<Result<<$command as Command>::Result, StorageError>>) {
                    let (response_channel, receiver) = oneshot::channel();

                    (
                        StorageCommand::$command(command, response_channel),
                        receiver
                    )
                }
            }
        )*
    };
}

// TODO: Replace by inventory
RegisterCommands!(AddPendingCertificate, CertificateDelivered, GetCertificate);

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
