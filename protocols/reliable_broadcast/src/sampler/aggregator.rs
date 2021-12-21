//!
//! Functionality to manage peers samples.
//!
use super::{sampling::sample_reduce_from, *};
use std::cmp::min;
use std::collections::HashSet;
use std::sync::{Arc, Mutex};
use tce_transport::{ReliableBroadcastParams, TrbpCommands, TrbpEvents};
use tokio::sync::broadcast;
use tokio::sync::mpsc;

#[derive(Debug)]
pub struct PeerSamplingOracle {
    pub events_subscribers: Vec<mpsc::UnboundedSender<TrbpEvents>>,
    pub sampling_commands_channel: mpsc::UnboundedSender<TrbpCommands>,
    pub trbp_params: ReliableBroadcastParams,
    pub visible_peers: Vec<Peer>,
    pub connected_peers: Vec<Peer>,

    echo_pending_subs: HashSet<Peer>,
    ready_pending_subs: HashSet<Peer>,
    delivery_pending_subs: HashSet<Peer>,

    pub view: SampleView,
    pub view_sender: broadcast::Sender<SampleView>,
    pub status: SampleProviderStatus,
}

impl PeerSamplingOracle {
    pub fn spawn_new(
        params: ReliableBroadcastParams,
        sample_view_sender: broadcast::Sender<SampleView>,
    ) -> Arc<Mutex<PeerSamplingOracle>> {
        let (s_command_sender, mut s_command_rcv) = mpsc::unbounded_channel::<TrbpCommands>();
        // Init the samples
        let mut default_view: SampleView = Default::default();
        default_view.insert(SampleType::EchoInbound, HashSet::new());
        default_view.insert(SampleType::EchoOutbound, HashSet::new());
        default_view.insert(SampleType::ReadyInbound, HashSet::new());
        default_view.insert(SampleType::ReadyOutbound, HashSet::new());
        default_view.insert(SampleType::DeliveryInbound, HashSet::new());

        let me = Arc::new(Mutex::from(Self {
            events_subscribers: Vec::new(),
            sampling_commands_channel: s_command_sender,
            trbp_params: params,
            visible_peers: vec![],
            connected_peers: vec![],
            echo_pending_subs: Default::default(),
            ready_pending_subs: Default::default(),
            delivery_pending_subs: Default::default(),
            view: default_view,
            view_sender: sample_view_sender,
            status: SampleProviderStatus::Stabilized,
        }));
        let me_cl = me.clone();
        tokio::spawn(async move {
            loop {
                tokio::select! {
                    // poll commands channel
                    cmd = s_command_rcv.recv() => {
                        Self::on_command(me_cl.clone(), cmd);
                    }
                }
            }
        });
        me
    }

    fn send_out_events(&mut self, evt: TrbpEvents) {
        for tx in &self.events_subscribers {
            // FIXME: When error is returned it means that receiving side of the channel is closed
            // Thus we better remove the sender from our subscribers
            let _ = tx.send(evt.clone());
        }
    }

    fn add_peer(&mut self, stype: SampleType, peer: &Peer) {
        self.view.get_mut(&stype).unwrap().insert(peer.clone());
    }

    /// Reaction to external events.
    /// We handle:
    /// - [TrbpCommands::OnVisiblePeersChanged] - used to initialise (or review) peers sets
    /// - [TrbpCommands::OnConnectedPeersChanged] - to keep the nearest nodes
    /// - [TrbpCommands::OnEchoSubscribeReq], [TrbpCommands::OnReadySubscribeReq] - to keep track of Inbound
    /// - [TrbpCommands::OnEchoSubscribeOk], [TrbpCommands::OnReadySubscribeOk] - to keep track of Outbound
    fn on_command(data: Arc<Mutex<PeerSamplingOracle>>, mb_cmd: Option<TrbpCommands>) {
        let mut aggr = data.lock().unwrap();
        match mb_cmd {
            Some(cmd) => {
                match cmd {
                    TrbpCommands::OnVisiblePeersChanged { peers } => {
                        // todo - properly react to small (not enough) network size
                        aggr.visible_peers = peers;
                        aggr.create_new_sample_view();
                    }
                    TrbpCommands::OnConnectedPeersChanged { peers } => {
                        aggr.connected_peers = peers.clone();
                    }
                    TrbpCommands::OnEchoSubscribeReq { from_peer } => {
                        aggr.add_peer(SampleType::EchoOutbound, &from_peer);
                        aggr.send_out_events(TrbpEvents::EchoSubscribeOk { to_peer: from_peer });
                    }
                    TrbpCommands::OnReadySubscribeReq { from_peer } => {
                        aggr.add_peer(SampleType::ReadyOutbound, &from_peer);
                        aggr.send_out_events(TrbpEvents::ReadySubscribeOk { to_peer: from_peer });
                    }
                    TrbpCommands::OnEchoSubscribeOk { from_peer } => {
                        if aggr.echo_pending_subs.remove(&from_peer) {
                            aggr.add_peer(SampleType::EchoInbound, &from_peer);
                        }
                    }
                    TrbpCommands::OnReadySubscribeOk { from_peer } => {
                        if aggr.delivery_pending_subs.contains(&from_peer)
                            && aggr.delivery_pending_subs.remove(&from_peer)
                        {
                            aggr.add_peer(SampleType::DeliveryInbound, &from_peer);
                        }
                        // Sampling with replacement, so can be both cases
                        if aggr.ready_pending_subs.contains(&from_peer)
                            && aggr.ready_pending_subs.remove(&from_peer)
                        {
                            aggr.add_peer(SampleType::ReadyInbound, &from_peer);
                        }
                    }
                    _ => {}
                }
            }
            _ => {
                log::warn!("empty command was passed");
            }
        }

        aggr.state_change_follow_up();
    }

    fn create_new_sample_view(&mut self) {
        self.status = SampleProviderStatus::BuildingNewView;

        // Init the samples
        self.view.insert(SampleType::EchoInbound, HashSet::new());
        self.view.insert(SampleType::EchoOutbound, HashSet::new());
        self.view.insert(SampleType::ReadyInbound, HashSet::new());
        self.view.insert(SampleType::ReadyOutbound, HashSet::new());
        self.view
            .insert(SampleType::DeliveryInbound, HashSet::new());

        self.init_echo_inbound_sample();
        self.init_ready_inbound_sample();
        self.init_delivery_inbound_sample();
    }

    fn state_change_follow_up(&mut self) {
        if matches!(self.status, SampleProviderStatus::Stabilized) {
            return;
        }

        let stable_view = self.echo_pending_subs.is_empty()
            && self.ready_pending_subs.is_empty()
            && self.delivery_pending_subs.is_empty()
            && !self.connected_peers.is_empty();

        if stable_view {
            // Attempt to send the new view to the Broadcaster
            match self.view_sender.send(self.view.clone()) {
                Ok(_) => {
                    self.status = SampleProviderStatus::Stabilized;
                }
                Err(e) => {
                    log::error!("Fail to send new sample view {:?} ", e);
                }
            }
        }
    }

    /// inbound echo sampling
    fn init_echo_inbound_sample(&mut self) {
        self.echo_pending_subs.clear();

        let echo_sizer = |len| min(len, self.trbp_params.echo_sample_size);
        let echo_candidates = sample_reduce_from(&self.visible_peers, echo_sizer)
            .expect("sampling echo")
            .value;

        for peer in &echo_candidates {
            self.echo_pending_subs.insert(peer.clone());
        }

        self.send_out_events(TrbpEvents::EchoSubscribeReq {
            peers: echo_candidates,
        });
    }

    /// inbound ready sampling
    fn init_ready_inbound_sample(&mut self) {
        self.ready_pending_subs.clear();

        let ready_sizer = |len| min(len, self.trbp_params.ready_sample_size);
        let ready_candidates = sample_reduce_from(&self.visible_peers, ready_sizer)
            .expect("sampling ready")
            .value;

        for peer in &ready_candidates {
            self.ready_pending_subs.insert(peer.clone());
        }

        self.send_out_events(TrbpEvents::ReadySubscribeReq {
            peers: ready_candidates,
        });
    }

    /// inbound delivery sampling
    fn init_delivery_inbound_sample(&mut self) {
        self.delivery_pending_subs.clear();

        let delivery_sizer = |len| min(len, self.trbp_params.delivery_sample_size.clone());
        let delivery_candidates = sample_reduce_from(&self.visible_peers, delivery_sizer)
            .expect("sampling delivery")
            .value;

        for peer in &delivery_candidates {
            self.delivery_pending_subs.insert(peer.clone());
        }

        self.send_out_events(TrbpEvents::ReadySubscribeReq {
            peers: delivery_candidates,
        });
    }
}
