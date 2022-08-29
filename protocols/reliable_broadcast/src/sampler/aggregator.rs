//!
//! Functionality to manage peers samples.
//!
use std::cmp::min;
use std::collections::HashSet;
use std::sync::{Arc, Mutex};

use log::info;
use tokio::sync::broadcast;
use tokio::sync::mpsc;

use tce_transport::{ReliableBroadcastParams, TrbpEvents};

use crate::{SamplerCommand, TrbInternalCommand};

use super::{sampling::sample_reduce_from, *};

#[derive(Debug)]
pub struct PeerSamplingOracle {
    pub events_subscribers: Vec<mpsc::UnboundedSender<TrbpEvents>>,
    pub sampling_commands_channel: mpsc::UnboundedSender<TrbInternalCommand>,
    pub trbp_params: ReliableBroadcastParams,
    pub visible_peers: Vec<Peer>,

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
        let (s_command_sender, mut s_command_rcv) = mpsc::unbounded_channel::<TrbInternalCommand>();
        // Init the samples
        let mut default_view: SampleView = Default::default();
        default_view.insert(SampleType::EchoSubscription, HashSet::new());
        default_view.insert(SampleType::EchoSubscriber, HashSet::new());
        default_view.insert(SampleType::ReadySubscription, HashSet::new());
        default_view.insert(SampleType::ReadySubscriber, HashSet::new());
        default_view.insert(SampleType::DeliverySubscription, HashSet::new());

        let me = Arc::new(Mutex::from(Self {
            events_subscribers: Vec::new(),
            sampling_commands_channel: s_command_sender,
            trbp_params: params,
            visible_peers: vec![],
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
        log::debug!("send_out_events(evt: {:?}", &evt);

        for tx in &self.events_subscribers {
            // FIXME: When error is returned it means that receiving side of the channel is closed
            // Thus we better remove the sender from our subscribers
            let _ = tx.send(evt.clone());
        }
    }

    fn add_confirmed_peer_to_sample(&mut self, stype: SampleType, peer: &Peer) {
        self.view.get_mut(&stype).unwrap().insert(peer.clone());
    }

    /// Reaction to external events.
    /// We handle:
    /// - [TrbpCommands::OnVisiblePeersChanged] - used to initialise (or review) peers sets
    /// - [TrbpCommands::OnConnectedPeersChanged] - to keep the nearest nodes
    /// - [TrbpCommands::OnEchoSubscribeReq], [TrbpCommands::OnReadySubscribeReq] - to keep track of Subscriptions
    /// - [TrbpCommands::OnEchoSubscribeOk], [TrbpCommands::OnReadySubscribeOk] - to keep track of Subscriber
    fn on_command(data: Arc<Mutex<PeerSamplingOracle>>, mb_cmd: Option<TrbInternalCommand>) {
        let mut aggr = data.lock().unwrap();
        match mb_cmd {
            Some(TrbInternalCommand::Sampler(command)) => match command {
                SamplerCommand::PeersChanged { peers } => {
                    if aggr.apply_visible_peers(peers) {
                        aggr.reset_subscription_samples();
                    }
                }
                SamplerCommand::ConfirmPeer { peer, sample_type } => {
                    info!("ConfirmPeer {peer} in {sample_type:?}");
                    match sample_type {
                        SampleType::EchoSubscription => {
                            if aggr.echo_pending_subs.remove(&peer) {
                                aggr.add_confirmed_peer_to_sample(
                                    SampleType::EchoSubscription,
                                    &peer,
                                );
                            }
                        }
                        SampleType::ReadySubscription => {
                            if aggr.ready_pending_subs.remove(&peer) {
                                aggr.add_confirmed_peer_to_sample(
                                    SampleType::ReadySubscription,
                                    &peer,
                                );
                            }
                        }
                        SampleType::DeliverySubscription => {
                            if aggr.delivery_pending_subs.remove(&peer) {
                                aggr.add_confirmed_peer_to_sample(
                                    SampleType::DeliverySubscription,
                                    &peer,
                                );
                            }
                        }

                        SampleType::EchoSubscriber => {
                            aggr.add_confirmed_peer_to_sample(SampleType::EchoSubscriber, &peer);
                            // notify the protocol that we updated Subscriber peers
                            aggr.view_sender.send(aggr.view.clone()).expect("send");
                        }
                        SampleType::ReadySubscriber => {
                            aggr.add_confirmed_peer_to_sample(SampleType::ReadySubscriber, &peer);
                            // notify the protocol that we updated Subscriber peers
                            aggr.view_sender.send(aggr.view.clone()).expect("send");
                        }
                    }
                }
            },

            _ => {
                log::warn!("empty command was passed");
            }
        }

        aggr.pending_subs_state_change_follow_up();
    }

    /// Returns true if the change is so significant that we need to recalculate samples
    fn apply_visible_peers(&mut self, new_peers: Vec<Peer>) -> bool {
        //todo check if some peers disappeared from the sets
        self.visible_peers = new_peers;
        true
    }

    fn reset_subscription_samples(&mut self) {
        self.status = SampleProviderStatus::BuildingNewView;

        // Init the samples
        self.view
            .insert(SampleType::EchoSubscription, HashSet::new());
        self.view
            .insert(SampleType::ReadySubscription, HashSet::new());
        self.view
            .insert(SampleType::DeliverySubscription, HashSet::new());

        self.reset_echo_subscription_sample();
        self.reset_ready_subscription_sample();
        self.reset_delivery_subscription_sample();
    }

    fn pending_subs_state_change_follow_up(&mut self) {
        if matches!(self.status, SampleProviderStatus::Stabilized) {
            return;
        }

        // todo - think about timeouts on Subscribe...
        let stable_view = self.echo_pending_subs.is_empty()
            && self.ready_pending_subs.is_empty()
            && self.delivery_pending_subs.is_empty();

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

    /// subscription echo sampling
    fn reset_echo_subscription_sample(&mut self) {
        self.echo_pending_subs.clear();

        let echo_sizer = |len| min(len, self.trbp_params.echo_sample_size);
        match sample_reduce_from(&self.visible_peers, echo_sizer) {
            Ok(echo_candidates) => {
                log::debug!(
                    "reset_echo_subscription_sample - echo_candidates: {:?}",
                    echo_candidates
                );

                for peer in &echo_candidates.value {
                    log::info!("Adding {peer} to pending echo subscriptions");
                    self.echo_pending_subs.insert(peer.clone());
                }

                self.send_out_events(TrbpEvents::EchoSubscribeReq {
                    peers: echo_candidates.value,
                });
            }
            Err(e) => {
                log::warn!(
                    "reset_echo_subscription_sample - failed to sample due to {:?}",
                    e
                );
            }
        }
    }

    /// subscription ready sampling
    fn reset_ready_subscription_sample(&mut self) {
        self.ready_pending_subs.clear();

        let ready_sizer = |len| min(len, self.trbp_params.ready_sample_size);
        match sample_reduce_from(&self.visible_peers, ready_sizer) {
            Ok(ready_candidates) => {
                log::debug!(
                    "reset_ready_subscription_sample - ready_candidates: {:?}",
                    ready_candidates
                );

                for peer in &ready_candidates.value {
                    log::info!("Adding {peer} to pending ready subscriptions");
                    self.ready_pending_subs.insert(peer.clone());
                }

                self.send_out_events(TrbpEvents::ReadySubscribeReq {
                    peers: ready_candidates.value,
                });
            }
            Err(e) => {
                log::warn!(
                    "reset_ready_subscription_sample - failed to sample due to {:?}",
                    e
                );
            }
        }
    }

    /// subscription delivery sampling
    fn reset_delivery_subscription_sample(&mut self) {
        self.delivery_pending_subs.clear();

        let delivery_sizer = |len| min(len, self.trbp_params.delivery_sample_size);
        match sample_reduce_from(&self.visible_peers, delivery_sizer) {
            Ok(delivery_candidates) => {
                log::debug!(
                    "reset_delivery_subscription_sample - delivery_candidates: {:?}",
                    delivery_candidates
                );

                for peer in &delivery_candidates.value {
                    log::info!("Adding {peer} to pending delivery subscriptions");
                    self.delivery_pending_subs.insert(peer.clone());
                }

                self.send_out_events(TrbpEvents::ReadySubscribeReq {
                    peers: delivery_candidates.value,
                });
            }
            Err(e) => {
                log::warn!(
                    "reset_delivery_subscription_sample - failed to sample due to {:?}",
                    e
                );
            }
        }
    }
}
