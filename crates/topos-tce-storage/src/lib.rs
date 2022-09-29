use std::{
    future::{Future, IntoFuture},
    time::Instant,
};

use futures::{future::BoxFuture, FutureExt};
use thiserror::Error;
use tokio::sync::mpsc;
use topos_core::uci::{Certificate, CertificateId, SubnetId};

#[async_trait::async_trait]
pub trait Storage: Send + 'static {
    async fn connect(&mut self) -> Result<(), StorageError>;
    /// Persist the certificate with given status
    async fn persist(
        &mut self,
        certificate: Certificate,
        status: CertificateStatus,
    ) -> Result<(), StorageError>;

    /// Update the certificate entry with new status
    async fn update(
        &mut self,
        certificate_id: &CertificateId,
        status: CertificateStatus,
    ) -> Result<(), StorageError>;

    /// Returns the tips of given subnets
    async fn get_tip(&self, subnets: Vec<SubnetId>) -> Result<Vec<Tip>, StorageError>;

    /// Returns the certificate data given their id
    async fn get_certificate_data(
        &self,
        cert_id: Vec<CertificateId>,
    ) -> Result<Vec<Certificate>, StorageError>;

    /// Returns the certificate emitted by given subnet
    /// Ranged by height since emitted Certificate are totally ordered
    async fn get_certificate_emitted(
        &self,
        subnet_id: SubnetId,
        from: Height,
        to: Height,
    ) -> Result<Vec<CertificateId>, StorageError>;

    /// Returns the certificate received by given subnet
    /// Ranged by timestamps since received Certificate are not referrable by height
    async fn get_certificate_received(
        &self,
        subnet_id: SubnetId,
        from: Instant,
        to: Instant,
    ) -> Result<Vec<CertificateId>, StorageError>;

    /// Returns all the known Certificate that are not delivered yet
    async fn get_certificate_pending(&self) -> Result<Vec<CertificateId>, StorageError>;
}

#[derive(Error, Debug)]
pub enum StorageError {
    #[error("The certificate already exists")]
    CertificateAlreadyExists,
}

/// Certificate index in the history of its emitter subnet
pub type Height = u64;

/// Uniquely identify the tip of which subnet
pub struct Tip {
    /// Certificate id of the tip
    cert_id: CertificateId,
    /// Subnet id of the tip
    subnet_id: SubnetId,
    /// Height of the Certificate
    height: Height,
    /// Timestamp of the Certificate
    timestamp: Instant,
}

pub enum CertificateStatus {
    Pending,
    Delivered,
}

pub enum StorageCommand {}

#[derive(Debug, Error)]
pub enum ConnectionError {
    #[error(transparent)]
    Storage(#[from] StorageError),
}

pub struct Connection<S: Storage> {
    storage: S,
    queries: mpsc::Receiver<StorageCommand>,
}

impl<S: Storage> Connection<S> {
    fn new(storage: S) -> (Self, mpsc::Sender<StorageCommand>) {
        let (sender, queries) = mpsc::channel(1024);

        (Self { storage, queries }, sender)
    }
}

impl<S: Storage> IntoFuture for Connection<S> {
    type Output = Result<(), ConnectionError>;

    type IntoFuture = BoxFuture<'static, Self::Output>;

    fn into_future(mut self) -> Self::IntoFuture {
        async move {
            self.storage.connect().await?;

            loop {
                tokio::select! {
                    command = self.queries.recv() => {

                    }
                }
            }

            Ok(())
        }
        .boxed()
    }
}
