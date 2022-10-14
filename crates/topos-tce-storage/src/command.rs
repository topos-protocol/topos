use tokio::sync::oneshot;
use topos_core::uci::{Certificate, CertificateId};

use crate::{errors::StorageError, PendingCertificateId};

#[derive(Debug)]
pub enum StorageCommand {
    AddPendingCertificate(
        AddPendingCertificate,
        oneshot::Sender<Result<<AddPendingCertificate as Command>::Result, StorageError>>,
    ),
    CertificateDelivered(
        CertificateDelivered,
        oneshot::Sender<Result<<CertificateDelivered as Command>::Result, StorageError>>,
    ),
    GetCertificate(
        GetCertificate,
        oneshot::Sender<Result<<GetCertificate as Command>::Result, StorageError>>,
    ),
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

#[derive(Debug)]
pub struct AddPendingCertificate {
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
    pub(crate) certificate_id: CertificateId,
}

impl Command for GetCertificate {
    type Result = Certificate;
}
