//! implementation of Topos Reliable Broadcast to be used in the Transmission Control Engine (TCE)
//!
//! Abstracted from actual transport implementation.
//! Abstracted from actual storage implementation.
//!
use futures::future::BoxFuture;
use sampler::SampleType;
use tokio::spawn;
use tokio_stream::wrappers::BroadcastStream;

use futures::{Stream, TryStreamExt};
#[allow(unused)]
use opentelemetry::global;
use tokio::sync::mpsc::Sender;
use tokio::sync::{broadcast, mpsc, oneshot};

use double_echo::aggregator::ReliableBroadcast;
use tce_transport::{ReliableBroadcastParams, TrbpCommands, TrbpEvents};

use topos_core::uci::{Certificate, CertificateId, SubnetId};

use crate::sampler::Sampler;
use crate::trb_store::TrbStore;
pub use topos_core::uci;
pub type Peer = String;

pub mod double_echo;
pub mod mem_store;
pub mod mock;
pub mod sampler;
pub mod trb_store;

/// Configuration of TRB implementation
pub struct ReliableBroadcastConfig {
    pub store: Box<dyn TrbStore + Send>,
    pub trbp_params: ReliableBroadcastParams,
    pub my_peer_id: Peer,
}

#[derive(Debug)]
pub enum SamplerCommand {
    PeersChanged {
        peers: Vec<String>,
    },
    ConfirmPeer {
        peer: String,
        sample_type: SampleType,
        sender: oneshot::Sender<Result<(), ()>>,
    },
}

#[derive(Debug)]
pub enum TrbInternalCommand {
    Command(TrbpCommands),

    Sampler(SamplerCommand),

    DeliveredCerts {
        subnet_id: SubnetId,
        limit: u64,
        sender: oneshot::Sender<Result<Vec<Certificate>, Errors>>,
    },
}

/// Thread safe client to the protocol aggregate
#[derive(Clone, Debug)]
pub struct ReliableBroadcastClient {
    #[allow(dead_code)]
    peer_id: String,
    broadcast_commands: mpsc::Sender<TrbInternalCommand>,
    sampling_commands: mpsc::Sender<SamplerCommand>,
}

impl ReliableBroadcastClient {
    /// Creates new instance of the aggregate and returns proxy to it.
    ///
    /// New client instances to the same aggregate can be cloned from the returned one.
    /// Aggregate is spawned as new task.
    pub fn new(
        config: ReliableBroadcastConfig,
    ) -> (Self, impl Stream<Item = Result<TrbpEvents, ()>>) {
        log::info!("new(trbp_params: {:?})", &config.trbp_params);

        let peer_id = config.my_peer_id.clone();

        let (sample_view_sender, sample_view_receiver) = mpsc::channel(2048);
        let (sampler_command_sender, command_receiver) = mpsc::channel(2048);
        let (event_sender, event_receiver) = broadcast::channel(2048);

        let sampler = Sampler::new(
            config.trbp_params.clone(),
            command_receiver,
            event_sender.clone(),
            sample_view_sender,
        );

        let b_w_aggr = ReliableBroadcast::spawn_new(config, sample_view_receiver);
        let mut b_aggr = b_w_aggr.lock().unwrap();
        let broadcast_commands = b_aggr.broadcast_commands_channel.clone();

        b_aggr.events_subscribers.push(event_sender);

        spawn(sampler.run());
        (
            Self {
                peer_id,
                broadcast_commands,
                sampling_commands: sampler_command_sender,
            },
            // TODO: Switch to bounded stream to better perf
            BroadcastStream::new(event_receiver).map_err(|_| ()),
        )
    }
    /// Schedule command for execution
    pub fn eval(&self, cmd: TrbpCommands) -> Result<(), Errors> {
        let is_broadcast_related = |cmd: &TrbpCommands| {
            matches!(
                cmd,
                TrbpCommands::StartUp
                    | TrbpCommands::Shutdown
                    | TrbpCommands::OnBroadcast { .. }
                    | TrbpCommands::OnGossip { .. }
                    | TrbpCommands::OnStartDelivery { .. }
                    | TrbpCommands::OnEcho { .. }
                    | TrbpCommands::OnReady { .. }
            )
        };
        if is_broadcast_related(&cmd) {
            log::debug!("eval for broadcast {:?}", cmd);
            self.broadcast_commands
                .try_send(TrbInternalCommand::Command(cmd))
                .map_err(|err| err.into())
        } else {
            Ok(())
        }
    }

    pub fn peer_changed(&self, peers: Vec<String>) {
        self.sampling_commands
            .try_send(SamplerCommand::PeersChanged { peers })
            .expect("Unable to send peer changed to sampler");
    }

    pub async fn add_confirmed_peer_to_sample(&self, sample_type: SampleType, peer: Peer) {
        let (sender, receiver) = oneshot::channel();

        self.sampling_commands
            .try_send(SamplerCommand::ConfirmPeer {
                peer,
                sample_type,
                sender,
            })
            .expect("Unable to send confirmation to sample");

        let _ = receiver.await.expect("Sender was dropped");
    }

    /// known peers
    /// todo: move it out somewhere out of here, use DHT to advertise urls of API nodes
    pub async fn known_peers_api_addrs(&self) -> Result<Vec<String>, Errors> {
        // todo
        Ok(vec![])
    }

    /// delivered certificates for given terminal chain after the given certificate
    pub fn delivered_certs(
        &self,
        subnet_id: SubnetId,
        _from_cert_id: CertificateId,
    ) -> BoxFuture<'static, Result<Vec<Certificate>, Errors>> {
        let (sender, receiver) = oneshot::channel();

        let broadcast_commands = self.broadcast_commands.clone();

        Box::pin(async move {
            let _ = broadcast_commands.send(TrbInternalCommand::DeliveredCerts {
                subnet_id,
                limit: 10,
                sender,
            });

            receiver.await.expect("Sender to be alive")
        })
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

    pub fn get_sampler_channel(&self) -> Sender<SamplerCommand> {
        self.sampling_commands.clone()
    }

    pub fn get_command_channels(&self) -> (Sender<SamplerCommand>, Sender<TrbInternalCommand>) {
        (
            self.sampling_commands.clone(),
            self.broadcast_commands.clone(),
        )
    }
}
/// Protocol and technical errors
#[derive(Debug)]
pub enum Errors {
    BadPeers {},
    BadCommand {},
    TokioError {},
    CertificateNotFound,
}

impl From<mpsc::error::SendError<TrbInternalCommand>> for Errors {
    fn from(_arg: mpsc::error::SendError<TrbInternalCommand>) -> Self {
        Errors::TokioError {}
    }
}

impl From<mpsc::error::TrySendError<TrbInternalCommand>> for Errors {
    fn from(_arg: mpsc::error::TrySendError<TrbInternalCommand>) -> Self {
        Errors::TokioError {}
    }
}
