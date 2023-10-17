//! implementation of Topos Reliable Broadcast to be used in the Transmission Control Engine (TCE)
//!
//! Abstracted from actual transport implementation.
//! Abstracted from actual storage implementation.

use double_echo::DoubleEcho;
use futures::Stream;
use sampler::SampleType;
use std::collections::HashSet;
use std::sync::Arc;
use tce_transport::{ProtocolEvents, ReliableBroadcastParams};
use thiserror::Error;
use tokio::spawn;
use tokio::sync::mpsc::Sender;
use tokio::sync::{broadcast, mpsc, oneshot};
use tokio_stream::wrappers::ReceiverStream;
use topos_core::types::ValidatorId;
use topos_core::uci::{Certificate, CertificateId};
use topos_crypto::messages::{MessageSigner, Signature};
use topos_metrics::DOUBLE_ECHO_COMMAND_CHANNEL_CAPACITY_TOTAL;
use topos_tce_storage::types::CertificateDeliveredWithPositions;
use topos_tce_storage::validator::ValidatorStore;
use tracing::{debug, error, info};

pub use topos_core::uci;

pub type Peer = String;

mod constant;
pub mod double_echo;
pub mod sampler;

#[cfg(feature = "task-manager-channels")]
pub mod task_manager_channels;
#[cfg(not(feature = "task-manager-channels"))]
pub mod task_manager_futures;

#[cfg(test)]
mod tests;

use crate::sampler::SubscriptionsView;

#[derive(Debug)]
pub enum TaskStatus {
    /// The task finished successfully and broadcasted the certificate + received ready
    Success,
    /// The task did not finish successfully and stopped.
    Failure,
}

/// Configuration of TCE implementation
pub struct ReliableBroadcastConfig {
    pub tce_params: ReliableBroadcastParams,
    pub validator_id: ValidatorId,
    pub validators: HashSet<ValidatorId>,
    pub message_signer: Arc<MessageSigner>,
}

#[derive(Debug)]
pub enum SamplerCommand {
    ValidatorChanged {
        validators: Vec<ValidatorId>,
    },
    ConfirmValidator {
        validator: ValidatorId,
        sample_type: SampleType,
        sender: oneshot::Sender<Result<(), ()>>,
    },
    ValidatorConfirmationFailed {
        validator: ValidatorId,
        sample_type: SampleType,
    },
    ForceResample,
}

#[derive(Debug, Clone)]
pub enum DoubleEchoCommand {
    /// Entry point for new certificate to submit as initial sender
    Broadcast {
        need_gossip: bool,
        cert: Certificate,
    },

    /// When echo reply received
    Echo {
        validator_id: ValidatorId,
        certificate_id: CertificateId,
        signature: Signature,
    },

    /// When ready reply received
    Ready {
        validator_id: ValidatorId,
        certificate_id: CertificateId,
        signature: Signature,
    },
}

/// Thread safe client to the protocol aggregate
#[derive(Clone, Debug)]
pub struct ReliableBroadcastClient {
    command_sender: Sender<DoubleEchoCommand>,
    pub(crate) double_echo_shutdown_channel: Sender<oneshot::Sender<()>>,
}

impl ReliableBroadcastClient {
    /// Creates new instance of the aggregate and returns proxy to it.
    ///
    /// New client instances to the same aggregate can be cloned from the returned one.
    /// Aggregate is spawned as new task.
    pub async fn new(
        config: ReliableBroadcastConfig,
        validator_store: Arc<ValidatorStore>,
        broadcast_sender: broadcast::Sender<CertificateDeliveredWithPositions>,
    ) -> (Self, impl Stream<Item = ProtocolEvents>) {
        let (event_sender, event_receiver) = mpsc::channel(*constant::PROTOCOL_CHANNEL_SIZE);
        let (command_sender, command_receiver) = mpsc::channel(*constant::COMMAND_CHANNEL_SIZE);
        let (double_echo_shutdown_channel, double_echo_shutdown_receiver) =
            mpsc::channel::<oneshot::Sender<()>>(1);

        let (task_manager_message_sender, task_manager_message_receiver) =
            mpsc::channel(*constant::BROADCAST_TASK_MANAGER_CHANNEL_SIZE);

        let double_echo = DoubleEcho::new(
            config.tce_params,
            config.validator_id,
            config.message_signer,
            config.validators,
            task_manager_message_sender,
            command_receiver,
            event_sender,
            double_echo_shutdown_receiver,
            validator_store,
            broadcast_sender,
        );

        spawn(double_echo.run(task_manager_message_receiver));

        (
            Self {
                command_sender,
                double_echo_shutdown_channel,
            },
            ReceiverStream::new(event_receiver),
        )
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

    #[error("Cannot create public address from private key")]
    ProducePublicAddress,

    #[error("Unable to execute shutdown for the reliable broadcast: {0}")]
    ShutdownCommunication(mpsc::error::SendError<oneshot::Sender<()>>),
}
