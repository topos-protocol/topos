use std::{
    collections::{HashMap, HashSet},
    future::IntoFuture,
    str::FromStr,
    sync::Arc,
};

use futures::{future::BoxFuture, FutureExt};
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;
use topos_core::{
    api::grpc::{
        self,
        shared::v1::Uuid as APIUuid,
        tce::v1::{
            CheckpointRequest, CheckpointResponse, FetchCertificatesRequest,
            FetchCertificatesResponse,
        },
    },
    errors::GrpcParsingError,
    types::ProofOfDelivery,
    uci::{Certificate, CertificateId, SubnetId},
};
use topos_p2p::{
    constant::SYNCHRONIZER_PROTOCOL, error::CommandExecutionError, NetworkClient, PeerId,
    RetryPolicy,
};
use topos_tce_gatekeeper::GatekeeperClient;
use topos_tce_storage::{errors::StorageError, store::ReadStore, validator::ValidatorStore};
use tracing::{debug, error, warn};
use uuid::Uuid;

mod config;
mod error;
#[cfg(test)]
mod tests;

pub use config::CheckpointsCollectorConfig;
pub use error::CheckpointsCollectorError;

pub struct CheckpointSynchronizer<G: GatekeeperClient, N: NetworkClient> {
    pub(crate) config: CheckpointsCollectorConfig,

    pub(crate) network: N,
    pub(crate) gatekeeper: G,
    #[allow(unused)]
    pub(crate) store: Arc<ValidatorStore>,

    pub(crate) current_request_id: Option<APIUuid>,

    pub(crate) shutdown: CancellationToken,

    #[allow(dead_code)]
    pub(crate) events: mpsc::Sender<CheckpointsCollectorEvent>,
}

impl<G: GatekeeperClient, N: NetworkClient> IntoFuture for CheckpointSynchronizer<G, N> {
    type Output = Result<(), CheckpointsCollectorError>;

    type IntoFuture = BoxFuture<'static, Self::Output>;

    fn into_future(mut self) -> Self::IntoFuture {
        async move {
            let mut interval = tokio::time::interval(tokio::time::Duration::from_secs(
                self.config.sync_interval_seconds,
            ));

            loop {
                tokio::select! {
                    _tick = interval.tick() => {
                        // On every tick, checking if there is a pending synchronization
                        // If there is, skip
                        // If there is not,
                        //  1. Ask a random peer for the diff between local and its latest checkpoint
                        //  2. Validate the PoD diff, if fail, go back to 1
                        //  3. Based on the diff, check if we already have some of the certs
                        //      - Fetch every missing certs from one peer
                        //      - Each certs triggers a precedence check
                        if self.current_request_id.is_none() {
                            if let Err(error) = self.initiate_request().await {
                                warn!("Unsuccessful sync due to: {}", error);
                            }
                        }
                    }

                    _ = self.shutdown.cancelled() => { break; }

                }
            }

            Ok(())
        }
        .boxed()
    }
}

#[derive(Debug, thiserror::Error)]
enum SyncError {
    #[error("Unable to fetch target peer from gatekeeper")]
    UnableToFetchTargetPeer,

    #[error("Unable to parse subnet id")]
    UnableToParseSubnetId,

    #[error("Gatekeeper returned no peer")]
    NoPeerAvailable,

    #[error(transparent)]
    GrpcParsingError(#[from] GrpcParsingError),

    #[error(transparent)]
    CertificateConversion(#[from] topos_core::api::grpc::shared::v1_conversions_certificate::Error),

    #[error(transparent)]
    SubnetConversion(#[from] topos_core::api::grpc::shared::v1_conversions_subnet::Error),

    #[error(transparent)]
    Network(#[from] CommandExecutionError),

    #[error(transparent)]
    Store(#[from] StorageError),
}

impl<G: GatekeeperClient, N: NetworkClient> CheckpointSynchronizer<G, N> {
    async fn ask_for_checkpoint(
        &self,
        peer: PeerId,
    ) -> Result<HashMap<SubnetId, Vec<ProofOfDelivery>>, SyncError> {
        let request_id: APIUuid = Uuid::new_v4().into();

        let checkpoint: Vec<grpc::tce::v1::ProofOfDelivery> = {
            let certificate_ids = self
                .store
                .get_checkpoint()?
                .values()
                .map(|head| head.certificate_id)
                .collect::<Vec<_>>();

            self.store
                .get_certificates(&certificate_ids[..])?
                .into_iter()
                .filter_map(|value| {
                    value.map(|delivered_certificate| delivered_certificate.proof_of_delivery)
                })
                .map(Into::into)
                .collect()
        };

        let req = CheckpointRequest {
            request_id: Some(request_id),
            checkpoint,
        };

        debug!("Asking {} for latest checkpoint", peer);
        let response: CheckpointResponse = self
            .network
            .send_request(peer, req, RetryPolicy::NoRetry, SYNCHRONIZER_PROTOCOL)
            .await?;

        let diff = response
            .checkpoint_diff
            .into_iter()
            .map(|v| {
                let subnet =
                    SubnetId::from_str(&v.key[..]).map_err(|_| SyncError::UnableToParseSubnetId)?;
                let proofs = v
                    .value
                    .into_iter()
                    .map(TryInto::try_into)
                    .collect::<Result<Vec<_>, _>>()?;
                Ok::<_, SyncError>((subnet, proofs))
            })
            .collect::<Result<HashMap<_, _>, _>>()?;

        Ok(diff)
    }

    fn insert_unverified_proofs(
        &self,
        diff: HashMap<SubnetId, Vec<ProofOfDelivery>>,
    ) -> Result<Vec<Vec<CertificateId>>, SyncError> {
        let mut certs: HashSet<CertificateId> = HashSet::new();
        for (_subnet, proofs) in diff {
            let len = proofs.len();
            let unverified_certs = self.store.insert_unverified_proofs(proofs)?;

            debug!("Persist {} unverified proofs", len);
            certs.extend(&unverified_certs[..]);
        }

        // Chunk certs
        let mut chunked_certs: Vec<Vec<CertificateId>> = vec![];

        let certs = certs.into_iter().collect::<Vec<_>>();

        for certs in certs.chunks(10) {
            chunked_certs.push(certs.to_vec());
        }

        Ok(chunked_certs)
    }

    async fn fetch_certificates(
        &self,
        certificate_ids: Vec<CertificateId>,
    ) -> Result<Vec<Certificate>, SyncError> {
        let target_peer = self
            .gatekeeper
            .get_random_peers(1)
            .await
            .map_err(|_| SyncError::UnableToFetchTargetPeer)
            .map(|peers| peers.last().cloned().ok_or(SyncError::NoPeerAvailable))??;

        let request_id: Option<APIUuid> = Some(Uuid::new_v4().into());
        let req = FetchCertificatesRequest {
            request_id,
            certificates: certificate_ids
                .iter()
                .map(|cert| (*cert.as_array()).into())
                .collect(),
        };

        debug!(
            "Ask {} for certificates payload: {:?}",
            target_peer, certificate_ids
        );
        let response = self
            .network
            .send_request::<_, FetchCertificatesResponse>(
                target_peer,
                req,
                RetryPolicy::NoRetry,
                SYNCHRONIZER_PROTOCOL,
            )
            .await?;

        let certificates: Result<Vec<Certificate>, _> = response
            .certificates
            .into_iter()
            .map(TryInto::try_into)
            .collect();

        Ok(certificates?)
    }

    async fn initiate_request(&mut self) -> Result<(), SyncError> {
        //  1. Ask a random peer for the diff between local and its latest checkpoint
        let target_peer = self
            .gatekeeper
            .get_random_peers(1)
            .await
            .map_err(|_| SyncError::UnableToFetchTargetPeer)
            .map(|peers| peers.last().cloned().ok_or(SyncError::NoPeerAvailable))??;

        let diff = self.ask_for_checkpoint(target_peer).await?;

        let certificates_to_catchup = self.insert_unverified_proofs(diff)?;

        for certificates in certificates_to_catchup {
            let certificates = self.fetch_certificates(certificates).await?;

            // TODO: verify every certificates
            for certificate in certificates {
                let store = self.store.clone();
                tokio::spawn(async move {
                    // Validate
                    // Check precedence
                    let certificate_id = certificate.id;
                    match store.synchronize_certificate(certificate).await {
                        Ok(_) => debug!("Certificate {} synchronized", certificate_id),
                        Err(e) => error!("Failed to sync because of: {:?}", e),
                    }
                });
            }
        }
        Ok(())
    }
}

pub enum CheckpointsCollectorEvent {}
