mod app_context;

use std::collections::{HashMap, VecDeque};
use std::future::IntoFuture;
use std::pin::Pin;
use std::task::Context;
use std::time::Duration;
use std::{future::Future, task::Poll};

pub use app_context::AppContext;
use futures::future::BoxFuture;
use futures::stream::FuturesUnordered;
use futures::{FutureExt, StreamExt};
use tokio::sync::mpsc::error::TrySendError;
use tokio::sync::{mpsc, oneshot};
use tokio::time::error::Elapsed;
use tokio::time::{timeout_at, Instant, Timeout};
use topos_core::uci::{Certificate, CertificateId};
use topos_tce_storage::{PendingCertificateId, StorageClient};
use tower::Service;
use tracing::error;

const MAX_PENDING_CERTIFICATES: usize = 1000;
const TTL_PENDING_CERTIFICATE: Duration = Duration::from_secs(30);

pub struct TCEStorage {
    connection: StorageClient,

    timeout: TimeoutCert,

    pending_mapping: HashMap<CertificateId, oneshot::Sender<()>>,
    pending_certificates: FuturesUnordered<PendingCertificate>,

    pending_queue: VecDeque<Certificate>,

    submitted_certificate: mpsc::Receiver<Certificate>,
    certificate_dispatcher: mpsc::Sender<Certificate>,

    notifier: mpsc::Receiver<DeliveredCert>,
}

pub type DeliveredCert = CertificateId;

#[pin_project::pin_project]
struct PendingCertificate {
    #[pin]
    resolver: tokio::time::Timeout<Pin<Box<dyn Future<Output = Certificate> + Send>>>,
    id: PendingCertificateId,
}

impl Future for PendingCertificate {
    type Output = Result<(PendingCertificateId, Certificate), Elapsed>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let mut this = self.project();

        if let Poll::Ready(import_result) = Pin::new(&mut this.resolver).poll_unpin(cx) {
            return Poll::Ready(import_result.map(|result| (*this.id, result)));
        }

        Poll::Pending
    }
}

pub struct StorageContext {
    pub certificate_submitter: mpsc::Sender<Certificate>,
    pub dispatchable_certificates: mpsc::Receiver<Certificate>,
    pub delivered_certificates: mpsc::Sender<DeliveredCert>,
}

impl TCEStorage {
    pub fn new(storage: StorageClient, ttl: Option<u64>) -> (Self, StorageContext) {
        let (certificate_submitter, submitted_certificate) = mpsc::channel(10);
        let (certificate_dispatcher, dispatchable_certificates) = mpsc::channel(10);
        let (delivered_certificates, notifier) = mpsc::channel(10);

        (
            Self {
                connection: storage,
                timeout: TimeoutCert {
                    ttl: ttl
                        .map(Duration::from_secs)
                        .unwrap_or(TTL_PENDING_CERTIFICATE),
                },
                pending_mapping: HashMap::new(),
                pending_certificates: FuturesUnordered::new(),
                pending_queue: VecDeque::new(),
                submitted_certificate,
                certificate_dispatcher,
                notifier,
            },
            StorageContext {
                certificate_submitter,
                dispatchable_certificates,
                delivered_certificates,
            },
        )
    }

    async fn add_pending_certificate(&mut self, certificate: Certificate) -> Result<(), ()> {
        if self.pending_certificates.len() > MAX_PENDING_CERTIFICATES {
            return Err(());
        }

        let pending_id = self
            .connection
            .persist_pending(certificate.clone())
            .await
            .unwrap();

        let resolver = if !certificate.prev_cert_id.is_empty() {
            let (sender, receiver) = oneshot::channel();

            self.pending_mapping
                .insert(certificate.prev_cert_id.clone(), sender);

            async move {
                let _: Result<(), _> = receiver.await;

                certificate
            }
            .boxed()
        } else {
            async move { certificate }.boxed()
        };

        let pending = PendingCertificate {
            resolver: self.timeout.call(resolver),
            id: pending_id,
        };

        self.pending_certificates.push(pending);
        Ok(())
    }

    fn handle_certificate_result(&mut self, certificate: Certificate) {
        self.pending_queue.push_back(certificate);
    }
}

impl IntoFuture for TCEStorage {
    type Output = Result<(), ()>;

    type IntoFuture = BoxFuture<'static, Self::Output>;

    fn into_future(mut self) -> Self::IntoFuture {
        async move {
            loop {
                while let Some(certificate) = self.pending_queue.pop_front() {
                    match self.certificate_dispatcher.try_send(certificate) {
                        Ok(_) => {}
                        Err(TrySendError::Full(certificate)) => {
                            // RuntimeQueue is full
                            self.pending_queue.push_front(certificate);
                            break;
                        }
                        Err(TrySendError::Closed(certificate)) => {
                            error!(
                                "Unable to send {:?} certificate to deliver process",
                                certificate.cert_id
                            );
                        }
                    }
                }

                tokio::select! {

                    Some(submitted_certificate) = self.submitted_certificate.recv() => {
                        if self.add_pending_certificate(submitted_certificate).await.is_err() {
                            error!("Can't add pending certificate to the storage");
                        }
                    }

                    Some(delivered_cert) = self.notifier.recv() => {
                        if let Some(resolver) = self.pending_mapping.remove(&delivered_cert) {
                            _ = resolver.send(());
                        }
                    }

                    certificate_result = self.pending_certificates.select_next_some() => {
                        match certificate_result {
                            Ok((_pending_id, certificate)) =>
                                self.handle_certificate_result(certificate),
                            Err(_) => {
                                println!("Certificate expired")
                            }
                        };
                    },
                }
            }
        }
        .boxed()
    }
}

struct TimeoutCert {
    ttl: Duration,
}

impl<O: Send, F: Future<Output = O>> Service<F> for TimeoutCert
where
    O: Send,
    F: Future<Output = O>,
{
    type Response = O;

    type Error = Elapsed;

    type Future = Timeout<F>;

    fn poll_ready(&mut self, _cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        Poll::Ready(Ok(()))
    }

    fn call(&mut self, req: F) -> Self::Future {
        timeout_at(Instant::now() + self.ttl, req)
    }
}
