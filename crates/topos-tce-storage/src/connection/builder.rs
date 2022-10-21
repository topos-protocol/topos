use std::future::Future;
use std::pin::Pin;
use std::{collections::VecDeque, sync::Arc};

use tokio::sync::mpsc;

use crate::{
    command::StorageCommand, errors::StorageError, events::StorageEvent, Connection,
    PendingCertificateId, Storage,
};

use super::MAX_PENDING_CERTIFICATES;

pub type StorageBuilder<S> = Pin<Box<dyn Future<Output = Result<S, StorageError>> + Send>>;

pub struct ConnectionBuilder<S: Storage> {
    pub(crate) storage_builder: Option<StorageBuilder<S>>,
    pub(crate) queries: mpsc::Receiver<StorageCommand>,
    pub(crate) events: mpsc::Sender<StorageEvent>,
    pub(crate) certificate_dispatcher: mpsc::Sender<PendingCertificateId>,
}

impl<S> ConnectionBuilder<S>
where
    S: Storage,
{
    /// Transforms a ConnectionBuilder into a Connection,
    /// applying the resolved Storage and the default options
    pub(crate) fn into_connection(self, storage: S) -> Connection<S> {
        Connection {
            storage: Arc::new(storage),
            queries: self.queries,
            events: self.events,
            certificate_dispatcher: Some(self.certificate_dispatcher),
            // TODO: Move MAX_PENDING_CERTIFICATES into a configuration option
            pending_certificates: VecDeque::with_capacity(MAX_PENDING_CERTIFICATES),
        }
    }
}
