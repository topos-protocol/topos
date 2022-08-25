//! implementation of Topos Reliable Broadcast to be used in the Transmission Control Engine (TCE)
//!
//! Abstracted from actual transport implementation.
//! Abstracted from actual storage implementation.
//!
use futures::future::BoxFuture;
use std::pin::Pin;
use std::sync::{Arc, Mutex};
use std::time;
use tokio_stream::wrappers::UnboundedReceiverStream;

use futures::{Future, Stream};
#[allow(unused)]
use opentelemetry::global;
use tokio::sync::mpsc::UnboundedSender;
use tokio::sync::{broadcast, mpsc, oneshot};

use double_echo::aggregator::ReliableBroadcast;
use sampler::{aggregator::PeerSamplingOracle, SampleView};
use tce_transport::{ReliableBroadcastParams, TrbpCommands, TrbpEvents};

use topos_core::uci::{Certificate, CertificateId, SubnetId};

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
pub enum TrbInternalCommand {
    Command(TrbpCommands),

    DeliveredCerts {
        subnet_id: SubnetId,
        limit: u64,
        sender: oneshot::Sender<Result<Vec<Certificate>, Errors>>,
    },
}

/// Thread safe client to the protocol aggregate
#[derive(Clone, Debug)]
pub struct ReliableBroadcastClient {
    peer_id: String,
    // b_aggr: Arc<Mutex<ReliableBroadcast>>,
    // s_aggr: Arc<Mutex<PeerSamplingOracle>>,
    broadcast_commands: mpsc::UnboundedSender<TrbInternalCommand>,
    sampling_commands: mpsc::UnboundedSender<TrbInternalCommand>,
}

pub struct ReliableBroadcastRuntime {
    peer_id: String,
    events: mpsc::UnboundedReceiver<TrbpEvents>,
}

impl ReliableBroadcastClient {
    /// Creates new instance of the aggregate and returns proxy to it.
    ///
    /// New client instances to the same aggregate can be cloned from the returned one.
    /// Aggregate is spawned as new task.
    pub fn new(config: ReliableBroadcastConfig) -> (Self, impl Stream<Item = TrbpEvents>) {
        log::info!("new(trbp_params: {:?})", &config.trbp_params);

        let peer_id = config.my_peer_id.clone();

        // Oneshot channel for new sample state (era)
        let (sample_view_sender, sample_view_receiver) = broadcast::channel::<SampleView>(16);

        let s_w_aggr =
            PeerSamplingOracle::spawn_new(config.trbp_params.clone(), sample_view_sender);
        let mut s_aggr = s_w_aggr.lock().unwrap();
        let sampling_commands = s_aggr.sampling_commands_channel.clone();

        let b_w_aggr = ReliableBroadcast::spawn_new(config, sample_view_receiver);
        let mut b_aggr = b_w_aggr.lock().unwrap();
        let broadcast_commands = b_aggr.broadcast_commands_channel.clone();

        let (events_sender, events_rcv) = mpsc::unbounded_channel::<TrbpEvents>();
        b_aggr.events_subscribers.push(events_sender.clone());
        s_aggr.events_subscribers.push(events_sender);

        // let runtime = ReliableBroadcastRuntime {
        //     peer_id: peer_id.clone(),
        //     events: events_rcv,
        // };

        (
            Self {
                peer_id,
                // b_aggr: b_w_aggr.clone(),
                // s_aggr: s_w_aggr.clone(),
                broadcast_commands,
                sampling_commands,
            },
            // TODO: Switch to bounded stream to better perf
            UnboundedReceiverStream::new(events_rcv),
        )
    }
    /// Schedule command for execution
    pub fn eval(&self, cmd: TrbpCommands) -> Result<(), Errors> {
        // FIXME: move the following operation to dedicated channel
        // match cmd {
        //     TrbpCommands::StartUp => {
        //         let mut b_aggr = self.b_aggr.lock().unwrap();
        //         b_aggr.on_cmd_start_up();
        //     }
        //     TrbpCommands::Shutdown => {
        //         let mut b_aggr = self.b_aggr.lock().unwrap();
        //         b_aggr.on_cmd_shut_down();
        //     }
        //     _ => {}
        // }

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
        let sender = if is_broadcast_related(&cmd) {
            log::debug!("eval for broadcast {:?}", cmd);
            &self.broadcast_commands
        } else {
            log::debug!("eval for sampling {:?}", cmd);
            &self.sampling_commands
        };

        sender
            .send(TrbInternalCommand::Command(cmd))
            .map_err(|err| err.into())
    }
    //
    // /// Pollable (in select!) events' listener
    // pub async fn next_event(&mut self) -> Result<TrbpEvents, Errors> {
    //     let event = self.events.recv().await;
    //     Ok(event.unwrap())
    // }

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
    pub fn delivered_certs_ids(
        &self,
        subnet_id: SubnetId,
        from_cert_id: CertificateId,
    ) -> BoxFuture<'static, Result<Vec<CertificateId>, Errors>> {
        let fut = self.delivered_certs(subnet_id, from_cert_id);

        Box::pin(async move { fut.await.map(|mut v| v.iter_mut().map(|c| c.id).collect()) })
    }

    pub fn get_command_channels(
        &self,
    ) -> (
        UnboundedSender<TrbInternalCommand>,
        UnboundedSender<TrbInternalCommand>,
    ) {
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

// impl Clone for ReliableBroadcastClient {
//     fn clone(&self) -> Self {
//         let mut b_aggr = self.b_aggr.lock().unwrap();
//         let ch_b_commands = b_aggr.broadcast_commands_channel.clone();
//
//         let mut s_aggr = self.s_aggr.lock().unwrap();
//         let ch_s_commands = s_aggr.sampling_commands_channel.clone();
//
//         let (events_sender, events_rcv) = mpsc::unbounded_channel::<TrbpEvents>();
//         b_aggr.events_subscribers.push(events_sender.clone());
//         s_aggr.events_subscribers.push(events_sender);
//         Self {
//             peer_id: self.peer_id.to_owned(),
//             b_aggr: self.b_aggr.clone(),
//             s_aggr: self.s_aggr.clone(),
//             broadcast_commands: ch_b_commands,
//             sampling_commands: ch_s_commands,
//             events: events_rcv,
//         }
//     }
// }
