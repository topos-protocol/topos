use tokio::sync::{mpsc, oneshot};
use topos_core::uci::{Certificate, CertificateId, SubnetId};

use crate::{
    errors::StorageError, CertificatePositions, FetchCertificatesFilter, FetchCertificatesPosition,
    PendingCertificateId,
};

use topos_commands::{Command, RegisterCommands};

// TODO: Replace by inventory
RegisterCommands!(
    name = StorageCommand,
    error = StorageError,
    commands = [
        AddPendingCertificate,
        CertificateDelivered,
        CheckPendingCertificateExists,
        GetCertificate,
        GetPendingCertificates,
        GetSourceHead,
        FetchCertificates,
        RemovePendingCertificate,
        TargetedBy
    ]
);

#[derive(Debug)]
pub struct AddPendingCertificate {
    #[allow(dead_code)]
    pub(crate) certificate: Certificate,
}

impl Command for AddPendingCertificate {
    type Result = PendingCertificateId;
}

#[derive(Debug)]
pub struct RemovePendingCertificate {
    pub(crate) pending_certificate_id: PendingCertificateId,
}

impl Command for RemovePendingCertificate {
    type Result = PendingCertificateId;
}

#[derive(Debug)]
pub struct GetPendingCertificates {}

impl Command for GetPendingCertificates {
    type Result = Vec<(PendingCertificateId, Certificate)>;
}

#[derive(Debug)]
pub struct CertificateDelivered {
    pub(crate) certificate_id: CertificateId,
}

impl Command for CertificateDelivered {
    // Return target and source stream positions for newly persisted certificate
    type Result = CertificatePositions;
}

#[derive(Debug)]
pub struct GetCertificate {
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
    type Result = Vec<(Certificate, FetchCertificatesPosition)>;
}

#[derive(Debug)]
pub struct GetSourceHead {
    pub(crate) subnet_id: SubnetId,
}

impl Command for GetSourceHead {
    type Result = (u64, Certificate);
}

#[derive(Debug)]
pub struct CheckPendingCertificateExists {
    pub(crate) certificate_id: CertificateId,
}

impl Command for CheckPendingCertificateExists {
    type Result = (PendingCertificateId, Certificate);
}

#[derive(Debug)]
pub struct TargetedBy {
    pub(crate) target_subnet_id: SubnetId,
}

impl Command for TargetedBy {
    type Result = Vec<SubnetId>;
}

#[cfg(test)]
mod tests {
    use std::time::Duration;
    use test_log::test;

    use tokio::spawn;
    use topos_test_sdk::constants::*;

    use super::*;

    #[test(tokio::test)]
    async fn send_command() {
        let cert = Certificate::new(
            PREV_CERTIFICATE_ID,
            SOURCE_SUBNET_ID_1,
            Default::default(),
            Default::default(),
            &[],
            0,
            Vec::new(),
        )
        .unwrap();
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
