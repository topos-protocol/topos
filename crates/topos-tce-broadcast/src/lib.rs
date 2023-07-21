//! implementation of Topos Reliable Broadcast to be used in the Transmission Control Engine (TCE)
//!
//! Abstracted from actual transport implementation.
//! Abstracted from actual storage implementation.

use std::collections::HashSet;

use sampler::SampleType;
use thiserror::Error;
use tokio::spawn;
use tokio_stream::wrappers::BroadcastStream;

use futures::{Stream, TryStreamExt};
use tokio::sync::mpsc::Sender;
use tokio::sync::{broadcast, mpsc, oneshot};

use double_echo::DoubleEcho;
use tce_transport::{ProtocolEvents, ReliableBroadcastParams};

use topos_core::uci::{Certificate, CertificateId};
use topos_metrics::DOUBLE_ECHO_COMMAND_CHANNEL_CAPACITY_TOTAL;
use topos_p2p::PeerId;
use topos_tce_storage::StorageClient;
use tracing::{debug, error, info};

pub use topos_core::uci;

pub type Peer = String;

mod constant;
pub mod double_echo;
pub mod sampler;

#[cfg(all(
    feature = "task-manager-channels",
    not(feature = "task-manager-futures")
))]
pub mod task_manager_channels;
#[cfg(feature = "task-manager-futures")]
pub mod task_manager_futures;

#[cfg(test)]
mod tests;

use crate::sampler::SubscriptionsView;

#[cfg(feature = "task-manager-futures")]
use crate::task_manager_futures::TaskManager;
#[cfg(all(
    feature = "task-manager-channels",
    not(feature = "task-manager-futures")
))]
use task_manager_channels::TaskManager;

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
    /// Entry point for new certificate to submit as initial sender
    Broadcast {
        need_gossip: bool,
        cert: Certificate,
    },

    /// When echo reply received
    Echo {
        from_peer: PeerId,
        certificate_id: CertificateId,
    },

    /// When ready reply received
    Ready {
        from_peer: PeerId,
        certificate_id: CertificateId,
    },
}

/// Thread safe client to the protocol aggregate
#[derive(Clone, Debug)]
pub struct ReliableBroadcastClient {
    command_sender: mpsc::Sender<DoubleEchoCommand>,
    pub(crate) subscriptions_view_sender: mpsc::Sender<SubscriptionsView>,
    pub(crate) double_echo_shutdown_channel: mpsc::Sender<oneshot::Sender<()>>,
}

impl ReliableBroadcastClient {
    /// Creates new instance of the aggregate and returns proxy to it.
    ///
    /// New client instances to the same aggregate can be cloned from the returned one.
    /// Aggregate is spawned as new task.
    pub async fn new(
        config: ReliableBroadcastConfig,
        local_peer_id: String,
        storage: StorageClient,
    ) -> (Self, impl Stream<Item = Result<ProtocolEvents, ()>>) {
        let (subscriptions_view_sender, subscriptions_view_receiver) =
            mpsc::channel::<SubscriptionsView>(2048);
        let (event_sender, event_receiver) = broadcast::channel(2048);
        let (command_sender, command_receiver) = mpsc::channel(*constant::COMMAND_CHANNEL_SIZE);
        let (double_echo_shutdown_channel, double_echo_shutdown_receiver) =
            mpsc::channel::<oneshot::Sender<()>>(1);

        let (task_manager_message_sender, task_manager_message_receiver) = mpsc::channel(24_000);
        let (task_completion_sender, task_completion_receiver) = mpsc::channel(24_000);

        let (task_manager, shutdown_receiver) = TaskManager::new(
            task_manager_message_receiver,
            task_completion_sender,
            config.tce_params,
        );

        spawn(task_manager.run(shutdown_receiver));

        let pending_certificate_count = storage
            .get_pending_certificates()
            .await
            .map(|v| v.len())
            .unwrap_or(0) as u64;

        let double_echo = DoubleEcho::new(
            task_manager_message_sender,
            task_completion_receiver,
            command_receiver,
            subscriptions_view_receiver,
            event_sender,
            double_echo_shutdown_receiver,
            local_peer_id,
            pending_certificate_count,
        );

        spawn(double_echo.run());

        (
            Self {
                command_sender,
                subscriptions_view_sender,
                double_echo_shutdown_channel,
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
                network_size: set.len(),
            })
            .await
            .map_err(|_| ())
    }

    pub fn get_double_echo_channel(&self) -> Sender<DoubleEchoCommand> {
        self.command_sender.clone()
    }

    /// Use to broadcast new certificate to the TCE network
    pub async fn broadcast_new_certificate(
        &self,
        certificate: Certificate,
        origin: bool,
    ) -> Result<(), ()> {
        let broadcast_commands = self.command_sender.clone();

        if broadcast_commands.capacity() <= *constant::COMMAND_CHANNEL_CAPACITY {
            DOUBLE_ECHO_COMMAND_CHANNEL_CAPACITY_TOTAL.inc();
        }

        info!("Send certificate to be broadcast");
        if broadcast_commands
            .send(DoubleEchoCommand::Broadcast {
                cert: certificate,
                need_gossip: origin,
            })
            .await
            .is_err()
        {
            error!("Unable to send broadcast_new_certificate command, Receiver was dropped");
        }

        Ok(())
    }

    pub async fn shutdown(&self) -> Result<(), Errors> {
        debug!("Shutting down reliable broadcast client");
        let (double_echo_sender, double_echo_receiver) = oneshot::channel();
        self.double_echo_shutdown_channel
            .send(double_echo_sender)
            .await
            .map_err(Errors::ShutdownCommunication)?;
        double_echo_receiver.await?;

        Ok(())
    }
}

/// Protocol and technical errors
#[derive(Error, Debug)]
pub enum Errors {
    #[error("Error while sending a DoubleEchoCommand to DoubleEcho: {0:?}")]
    DoubleEchoSend(#[from] Box<mpsc::error::SendError<DoubleEchoCommand>>),

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
