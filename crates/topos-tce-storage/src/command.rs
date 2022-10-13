use tokio::sync::oneshot;
use topos_core::uci::{Certificate, CertificateId, SubnetId};

use crate::{errors::StorageError, CertificateStatus, PendingCertificateId};

#[derive(Debug)]
pub enum StorageCommand {
    Persist {
        certificate: Certificate,
        status: CertificateStatus,
        response_channel: oneshot::Sender<Result<PendingCertificateId, StorageError>>,
    },

    UpdateCertificate {
        certificate_id: CertificateId,
        status: CertificateStatus,
        response_channel: oneshot::Sender<Result<(), StorageError>>,
    },

    ReadStream {
        subnet_id: SubnetId,
        from: ExpectedVersion,
        limit: u64,
        response_channel: oneshot::Sender<Result<Vec<Certificate>, StorageError>>,
    },

    GetCertificate {
        certificate_id: CertificateId,
        response_channel: oneshot::Sender<Result<Certificate, StorageError>>,
    },
}

pub trait Command {
    type Result: 'static;
}

#[derive(Debug)]
pub enum ExpectedVersion {
    Version(u64),
    Start,
    End,
}
