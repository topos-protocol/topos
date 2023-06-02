//! implementation of Topos Reliable Broadcast to be used in the Transmission Control Engine (TCE)
//!
//! Abstracted from actual transport implementation.
//! Abstracted from actual storage implementation.

use std::collections::HashSet;

use sampler::SampleType;
use thiserror::Error;
use tokio::spawn;
use tokio_stream::wrappers::BroadcastStream;

use futures::{Future, Stream, TryStreamExt};
#[allow(unused)]
use opentelemetry::global;
use tokio::sync::mpsc::Sender;
use tokio::sync::{broadcast, mpsc, oneshot};

use double_echo::DoubleEcho;
use tce_transport::{ProtocolEvents, ReliableBroadcastParams};

use topos_core::uci::{Certificate, CertificateId, SubnetId};
use topos_p2p::PeerId;
use topos_tce_storage::StorageClient;
use tracing::{debug, error, info, Span};

use crate::mem_store::TceMemStore;
use crate::sampler::SubscriptionsView;
use crate::tce_store::TceStore;
pub use topos_core::uci;

pub type Peer = String;

pub mod double_echo;
pub mod mem_store;
pub mod sampler;
pub mod tce_store;

#[cfg(test)]
mod tests;

/// Configuration of TCE implementation
pub struct ReliableBroadcastConfig {
    pub tce_params: ReliableBroadcastParams,
}

#[derive(Debug)]
pub enum SamplerCommand {
    PeersChanged {
        peers: Vec<PeerId>,
    },
    ConfirmPeer {
        peer: PeerId,
        sample_type: SampleType,
        sender: oneshot::Sender<Result<(), ()>>,
    },
    PeerConfirmationFailed {
        peer: PeerId,
        sample_type: SampleType,
    },
    ForceResample,
}

#[derive(Debug)]
pub enum DoubleEchoCommand {
    IsCertificateDelivered {
        certificate_id: CertificateId,
        sender: oneshot::Sender<bool>,
    },

    GetSpanOfCert {
        certificate_id: CertificateId,
        sender: oneshot::Sender<Result<Span, Errors>>,
    },

    /// Received G-set message
    Deliver {
        from_peer: PeerId,
        certificate_id: CertificateId,
        ctx: Span,
    },

    /// Entry point for new certificate to submit as initial sender
    Broadcast {
        origin: bool,
        cert: Certificate,
        ctx: Span,
    },

    /// When echo reply received
    Echo {
        from_peer: PeerId,
        certificate_id: CertificateId,
        ctx: Span,
    },

    /// When ready reply received
    Ready {
        from_peer: PeerId,
        certificate_id: CertificateId,
        ctx: Span,
    },
    DeliveredCerts {
        subnet_id: SubnetId,
        limit: u64,
        sender: oneshot::Sender<Result<Vec<Certificate>, Errors>>,
    },
}

/// Thread safe client to the protocol aggregate
#[derive(Clone, Debug)]
pub struct ReliableBroadcastClient {
    broadcast_commands: mpsc::Sender<DoubleEchoCommand>,
    // sampling_commands: mpsc::Sender<SamplerCommand>,
    pub(crate) subscriptions_view_sender: mpsc::Sender<SubscriptionsView>,
    // subscribers_update_sender: mpsc::Sender<SubscribersUpdate>,
    pub(crate) double_echo_shutdown_channel: mpsc::Sender<oneshot::Sender<()>>,
    // pub(crate) sampler_shutdown_channel: mpsc::Sender<oneshot::Sender<()>>,
}

impl ReliableBroadcastClient {
    /// Creates new instance of the aggregate and returns proxy to it.
    ///
    /// New client instances to the same aggregate can be cloned from the returned one.
    /// Aggregate is spawned as new task.
    // #[instrument(name = "ReliableBroadcastClient", skip_all)]
    pub async fn new(
        config: ReliableBroadcastConfig,
        local_peer_id: String,
        storage: StorageClient,
        network_client: topos_p2p::Client,
    ) -> (Self, impl Stream<Item = Result<ProtocolEvents, ()>>) {
        let (subscriptions_view_sender, subscriptions_view_receiver) =
            mpsc::channel::<SubscriptionsView>(2048);
        // let (subscribers_update_sender, subscribers_update_receiver) =
        //     mpsc::channel::<SubscribersUpdate>(2048);
        // let (sampler_command_sender, command_receiver) = mpsc::channel(2048);
        let (event_sender, event_receiver) = broadcast::channel(2048);

        // let (sampler_shutdown_channel, sampler_shutdown_receiver) =
        //     mpsc::channel::<oneshot::Sender<()>>(1);

        // let sampler = Sampler::new(
        //     config.tce_params.clone(),
        //     command_receiver,
        //     event_sender.clone(),
        //     subscriptions_view_sender,
        //     subscribers_update_sender,
        //     sampler_shutdown_receiver,
        // );

        let (broadcast_commands, command_receiver) = mpsc::channel(2048);
        let (double_echo_shutdown_channel, double_echo_shutdown_receiver) =
            mpsc::channel::<oneshot::Sender<()>>(1);

        let last_pending_certificate = storage
            .next_pending_certificate(None)
            .await
            .map(|(id, _)| id)
            .unwrap_or(0);

        let double_echo = DoubleEcho::new(
            config.tce_params,
            command_receiver,
            subscriptions_view_receiver,
            // subscribers_update_receiver,
            event_sender,
            #[allow(clippy::box_default)]
            Box::new(TceMemStore::default()),
            storage,
            network_client,
            double_echo_shutdown_receiver,
            local_peer_id,
            last_pending_certificate,
            std::env::var("TOPOS_BROADCAST_MAX_BUFFER_SIZE")
                .ok()
                .and_then(|s| s.parse().ok())
                .unwrap_or(DoubleEcho::MAX_BUFFER_SIZE),
        );

        // spawn(sampler.run());
        spawn(double_echo.run());

        (
            Self {
                broadcast_commands,
                subscriptions_view_sender,
                // subscribers_update_sender,
                // sampling_commands: sampler_command_sender,
                double_echo_shutdown_channel,
                // sampler_shutdown_channel,
            },
            BroadcastStream::new(event_receiver).map_err(|_| ()),
        )
    }

    pub async fn peer_changed(&self, peers: Vec<PeerId>) -> Result<(), ()> {
        let set = peers.into_iter().collect::<HashSet<_>>();
        self.subscriptions_view_sender
            .send(SubscriptionsView {
                echo: set.clone(),
                ready: set.clone(),
                delivery: set.clone(),
                network_size: set.len(),
            })
            .await
            .map_err(|_| ())
        // let command_channel = self.get_double_echo_channel();
        // async move {
        //     if command_channel
        //         .send(SamplerCommand::PeersChanged { peers })
        //         .await
        //         .is_err()
        //     {
        //         error!("Unable to send peer changed to sampler");
        //     }
        //     Ok(())
        // }
    }

    // pub async fn force_resample(&self) {
    //     _ = self
    //         .get_sampler_channel()
    //         .send(SamplerCommand::ForceResample)
    //         .await;
    // }

    pub async fn add_confirmed_peer_to_sample(&self, _sample_type: SampleType, _peer: PeerId) {
        // let (sender, receiver) = oneshot::channel();
        //
        // if self
        //     .sampling_commands
        //     .send(SamplerCommand::ConfirmPeer {
        //         peer,
        //         sample_type,
        //         sender,
        //     })
        //     .await
        //     .is_err()
        // {
        //     error!("Unable to send confirmation to sample");
        // }
        //
        // if receiver.await.is_err() {
        //     error!("Unable to receive add_confirmed_peer_to_sample response, Sender was dropped");
        // }
    }

    /// delivered certificates for given target chain after the given certificate
    pub fn delivered_certs(
        &self,
        subnet_id: SubnetId,
        _from_cert_id: CertificateId,
    ) -> impl Future<Output = Result<Vec<Certificate>, Errors>> + 'static + Send {
        let (sender, receiver) = oneshot::channel();

        let broadcast_commands = self.broadcast_commands.clone();

        async move {
            if broadcast_commands
                .send(DoubleEchoCommand::DeliveredCerts {
                    subnet_id,
                    limit: 10,
                    sender,
                })
                .await
                .is_err()
            {
                error!("Unable to execute delivered_certs");
            }

            receiver.await.map_err(Into::into).and_then(|result| result)
        }
    }

    /// delivered certificates for given target chain after the given certificate
    pub async fn get_span_cert(&self, certificate_id: CertificateId) -> Result<Span, Errors> {
        let (sender, receiver) = oneshot::channel();

        let broadcast_commands = self.broadcast_commands.clone();

        if broadcast_commands
            .send(DoubleEchoCommand::GetSpanOfCert {
                certificate_id,
                sender,
            })
            .await
            .is_err()
        {
            error!("Unable to execute get_span_cert");
        }

        receiver.await.map_err(Into::into).and_then(|result| result)
    }

    pub async fn delivered_certs_ids(
        &self,
        subnet_id: SubnetId,
        from_cert_id: CertificateId,
    ) -> Result<Vec<CertificateId>, Errors> {
        self.delivered_certs(subnet_id, from_cert_id)
            .await
            .map(|mut v| v.iter_mut().map(|c| c.id).collect())
    }

    // pub fn get_sampler_channel(&self) -> Sender<SamplerCommand> {
    //     self.sampling_commands.clone()
    // }

    pub fn get_double_echo_channel(&self) -> Sender<DoubleEchoCommand> {
        self.broadcast_commands.clone()
    }

    // pub fn get_command_channels(&self) -> (Sender<SamplerCommand>, Sender<DoubleEchoCommand>) {
    //     (
    //         self.sampling_commands.clone(),
    //         self.broadcast_commands.clone(),
    //     )
    // }

    /// Use to broadcast new certificate to the TCE network
    pub async fn broadcast_new_certificate(
        &self,
        certificate: Certificate,
        origin: bool,
    ) -> Result<(), ()> {
        let broadcast_commands = self.broadcast_commands.clone();

        info!("Send certificate to be broadcast");
        if broadcast_commands
            .send(DoubleEchoCommand::Broadcast {
                cert: certificate,
                origin,
                ctx: Span::current(),
            })
            .await
            .is_err()
        {
            error!("Unable to send broadcast_new_certificate command, Receiver was dropped");
        }

        Ok(())
    }

    pub async fn is_certificate_delivered_in_cache(
        &self,
        certificate_id: CertificateId,
    ) -> Result<bool, Errors> {
        let (sender, receiver) = oneshot::channel();

        let broadcast_commands = self.broadcast_commands.clone();

        if broadcast_commands
            .send(DoubleEchoCommand::IsCertificateDelivered {
                certificate_id,
                sender,
            })
            .await
            .is_err()
        {
            error!("Unable to send is_certificate_delivered command, Receiver was dropped");
        }

        receiver.await.map_err(Into::into)
    }

    pub async fn shutdown(&self) -> Result<(), Errors> {
        debug!("Shutting down reliable broadcast client");
        let (double_echo_sender, double_echo_receiver) = oneshot::channel();
        self.double_echo_shutdown_channel
            .send(double_echo_sender)
            .await
            .map_err(Errors::ShutdownCommunication)?;
        double_echo_receiver.await?;

        // let (sampler_sender, sampler_receiver) = oneshot::channel();
        // self.sampler_shutdown_channel
        //     .send(sampler_sender)
        //     .await
        //     .map_err(Errors::ShutdownCommunication)?;
        // Ok(sampler_receiver.await?)
        Ok(())
    }
}

/// Protocol and technical errors
#[derive(Error, Debug)]
pub enum Errors {
    #[error("Error while sending a DoubleEchoCommand to DoubleEcho: {0:?}")]
    DoubleEchoSend(#[from] mpsc::error::SendError<DoubleEchoCommand>),

    #[error("Error while waiting for a DoubleEchoCommand response: {0:?}")]
    DoubleEchoRecv(#[from] oneshot::error::RecvError),

    #[error("Error while sending a SamplerCommand to Sampler: {0:?}")]
    SamplerSend(#[from] mpsc::error::SendError<SamplerCommand>),

    #[error("Requested certificate not found")]
    CertificateNotFound,

    #[error("Requested digest not found for certificate {0:?}")]
    DigestNotFound(CertificateId),

    #[error("Unable to execute shutdown for the reliable broadcast: {0}")]
    ShutdownCommunication(mpsc::error::SendError<oneshot::Sender<()>>),
}
