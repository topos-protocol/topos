mod cyclerng;
mod sampling;

// Move to transport crate
pub type Peer = String;
use std::{
    cmp::min,
    collections::{HashMap, HashSet},
};

use log::info;
use tce_transport::{ReliableBroadcastParams, TrbpEvents};
use tokio::sync::{broadcast, mpsc};

use crate::SamplerCommand;

use self::sampling::sample_reduce_from;

#[derive(Debug, Default)]
pub struct ThresholdConfig {
    pub echo: usize,
    pub ready: usize,
    pub delivery: usize,
}

#[derive(Debug)]
pub enum SampleProviderStatus {
    /// The view on the peers changed hence building new samples
    BuildingNewView,
    /// The last request for sample build is done
    Stabilized,
}

#[derive(PartialEq, Eq, Hash, Debug, Clone)]
pub enum SampleType {
    /// Inbound: FROM external peer TO me
    /// Message from those I am following
    EchoSubscription,
    ReadySubscription,
    DeliverySubscription,

    /// Outbound: FROM me TO external peer
    /// Message to my followers
    EchoSubscriber,
    ReadySubscriber,
}

//#[derive(Debug, Default)]
pub type SampleView = HashMap<SampleType, HashSet<Peer>>;

pub struct Sampler {
    params: ReliableBroadcastParams,
    command_receiver: mpsc::Receiver<SamplerCommand>,
    event_sender: broadcast::Sender<TrbpEvents>,
    visible_peers: Vec<Peer>,

    pending_echo_subscribtions: HashSet<Peer>,
    pending_ready_subscribtions: HashSet<Peer>,
    pending_delivery_subscribtions: HashSet<Peer>,

    view: SampleView,
    view_sender: mpsc::Sender<SampleView>,

    status: SampleProviderStatus,
}

impl Sampler {
    pub fn new(
        params: ReliableBroadcastParams,
        command_receiver: mpsc::Receiver<SamplerCommand>,
        event_sender: broadcast::Sender<TrbpEvents>,
        view_sender: mpsc::Sender<SampleView>,
    ) -> Self {
        Self {
            params,
            command_receiver,
            event_sender,
            view_sender,
            visible_peers: Vec::new(),
            pending_echo_subscribtions: HashSet::new(),
            pending_ready_subscribtions: HashSet::new(),
            pending_delivery_subscribtions: HashSet::new(),
            view: SampleView::new(),
            status: SampleProviderStatus::Stabilized,
        }
    }

    /// Reaction to external events.
    /// We handle:
    /// - [TrbpCommands::OnVisiblePeersChanged] - used to initialise (or review) peers sets
    /// - [TrbpCommands::OnConnectedPeersChanged] - to keep the nearest nodes
    /// - [TrbpCommands::OnEchoSubscribeReq], [TrbpCommands::OnReadySubscribeReq] - to keep track of Subscriptions
    /// - [TrbpCommands::OnEchoSubscribeOk], [TrbpCommands::OnReadySubscribeOk] - to keep track of Subscriber
    pub async fn run(mut self) {
        loop {
            tokio::select! {
                Some(command) = self.command_receiver.recv() => {
                    match command {
                        SamplerCommand::ConfirmPeer { peer, sample_type, sender } => {
                            let _ = match self.handle_peer_confirmation(sample_type, peer).await {
                                Ok(_) => sender.send(Ok(())),
                                Err(error) => sender.send(Err(error)),
                            };

                            self.pending_subs_state_change_follow_up().await;
                        }
                        SamplerCommand::PeersChanged { peers } => self.peer_changed(peers)
                    }
                }
            }
        }
    }
}

impl Sampler {
    pub(crate) fn add_confirmed_peer_to_sample(
        &mut self,
        sample_type: SampleType,
        peer: &str,
    ) -> Result<(), ()> {
        if self
            .view
            .entry(sample_type)
            .or_default()
            .insert(peer.to_string())
        {
            Ok(())
        } else {
            Err(())
        }
    }

    fn peer_changed(&mut self, peers: Vec<Peer>) {
        self.visible_peers = peers;
        self.reset_subscription_samples();
    }

    async fn pending_subs_state_change_follow_up(&mut self) {
        if matches!(self.status, SampleProviderStatus::Stabilized) {
            return;
        }

        // todo - think about timeouts on Subscribe...
        let stable_view = self.pending_echo_subscribtions.is_empty()
            && self.pending_ready_subscribtions.is_empty()
            && self.pending_ready_subscribtions.is_empty();

        if stable_view {
            // Attempt to send the new view to the Broadcaster
            match self.view_sender.send(self.view.clone()).await {
                Ok(_) => {
                    self.status = SampleProviderStatus::Stabilized;
                }
                Err(e) => {
                    log::error!("Fail to send new sample view {:?} ", e);
                }
            }
        }
    }
    async fn handle_peer_confirmation(
        &mut self,
        sample_type: SampleType,
        peer: Peer,
    ) -> Result<(), ()> {
        info!("ConfirmPeer {peer} in {sample_type:?}");
        match sample_type {
            SampleType::EchoSubscription => {
                if self.pending_echo_subscribtions.remove(&peer) {
                    return self.add_confirmed_peer_to_sample(SampleType::EchoSubscription, &peer);
                }
            }
            SampleType::ReadySubscription => {
                if self.pending_ready_subscribtions.remove(&peer) {
                    return self.add_confirmed_peer_to_sample(SampleType::ReadySubscription, &peer);
                }
            }
            SampleType::DeliverySubscription => {
                if self.pending_delivery_subscribtions.remove(&peer) {
                    return self
                        .add_confirmed_peer_to_sample(SampleType::DeliverySubscription, &peer);
                }
            }

            SampleType::EchoSubscriber => {
                let _ = self.add_confirmed_peer_to_sample(SampleType::EchoSubscriber, &peer);
                // notify the protocol that we updated Subscriber peers
                return self
                    .view_sender
                    .send(self.view.clone())
                    .await
                    .map_err(|_| ());
            }
            SampleType::ReadySubscriber => {
                let _ = self.add_confirmed_peer_to_sample(SampleType::ReadySubscriber, &peer);
                // notify the protocol that we updated Subscriber peers
                return self
                    .view_sender
                    .send(self.view.clone())
                    .await
                    .map_err(|_| ());
            }
        }

        Err(())
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

    /// subscription echo sampling
    fn reset_echo_subscription_sample(&mut self) {
        self.pending_echo_subscribtions.clear();

        let echo_sizer = |len| min(len, self.params.echo_sample_size);
        match sample_reduce_from(&self.visible_peers, echo_sizer) {
            Ok(echo_candidates) => {
                log::debug!(
                    "reset_echo_subscription_sample - echo_candidates: {:?}",
                    echo_candidates
                );

                for peer in &echo_candidates.value {
                    log::info!("Adding {peer} to pending echo subscriptions");
                    self.pending_echo_subscribtions.insert(peer.clone());
                }

                if let Err(error) = self.event_sender.send(TrbpEvents::EchoSubscribeReq {
                    peers: echo_candidates.value,
                }) {
                    log::error!("Unable to send event {:?}", error);
                }
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
        self.pending_ready_subscribtions.clear();

        let ready_sizer = |len| min(len, self.params.ready_sample_size);
        match sample_reduce_from(&self.visible_peers, ready_sizer) {
            Ok(ready_candidates) => {
                log::debug!(
                    "reset_ready_subscription_sample - ready_candidates: {:?}",
                    ready_candidates
                );

                for peer in &ready_candidates.value {
                    log::info!("Adding {peer} to pending ready subscriptions");
                    self.pending_ready_subscribtions.insert(peer.clone());
                }

                if let Err(error) = self.event_sender.send(TrbpEvents::ReadySubscribeReq {
                    peers: ready_candidates.value,
                }) {
                    log::error!("Unable to send event {:?}", error);
                }
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
        self.pending_delivery_subscribtions.clear();

        let delivery_sizer = |len| min(len, self.params.delivery_sample_size);
        match sample_reduce_from(&self.visible_peers, delivery_sizer) {
            Ok(delivery_candidates) => {
                log::debug!(
                    "reset_delivery_subscription_sample - delivery_candidates: {:?}",
                    delivery_candidates
                );

                for peer in &delivery_candidates.value {
                    log::info!("Adding {peer} to pending delivery subscriptions");
                    self.pending_delivery_subscribtions.insert(peer.clone());
                }

                if let Err(error) = self.event_sender.send(TrbpEvents::ReadySubscribeReq {
                    peers: delivery_candidates.value,
                }) {
                    log::error!("Unable to send event {:?}", error);
                }
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

#[derive(Debug)]
pub struct SamplerClient {
    #[allow(dead_code)]
    pub(crate) command: mpsc::Sender<SamplerCommand>,
}

#[cfg(test)]
mod tests {

    use tokio::sync::broadcast::error::TryRecvError;

    use super::*;

    #[test]
    fn on_peer_change_sample_view_is_reset() {
        cyclerng::utils::set_cycle([1]);

        let (_, cmd_receiver) = mpsc::channel(10);
        let (event_sender, mut event_receiver) = broadcast::channel(10);
        let (view_sender, _) = mpsc::channel(10);

        let g = |a, b| ((a as f32) * b) as usize;
        let mut sampler = Sampler::new(
            ReliableBroadcastParams {
                echo_threshold: g(1, 0.66),
                echo_sample_size: 1,
                ready_threshold: g(1, 0.33),
                ready_sample_size: 1,
                delivery_threshold: g(1, 0.66),
                delivery_sample_size: 1,
            },
            cmd_receiver,
            event_sender,
            view_sender,
        );

        let mut peers = Vec::new();
        for i in 0..=100 {
            peers.push(format!("peer_{i}"));
        }

        sampler.peer_changed(peers);

        assert_eq!(sampler.pending_echo_subscribtions.len(), 1);
        assert_eq!(sampler.pending_ready_subscribtions.len(), 1);
        assert_eq!(sampler.pending_delivery_subscribtions.len(), 1);

        assert_eq!(event_receiver.len(), 3);
        assert!(matches!(
            event_receiver.try_recv(),
            Ok(TrbpEvents::EchoSubscribeReq { .. })
        ));
        assert!(matches!(
            event_receiver.try_recv(),
            Ok(TrbpEvents::ReadySubscribeReq { .. })
        ));
        assert!(matches!(
            event_receiver.try_recv(),
            Ok(TrbpEvents::ReadySubscribeReq { .. })
        ));
        assert!(matches!(
            event_receiver.try_recv(),
            Err(TryRecvError::Empty)
        ));

        assert_eq!(
            sampler
                .view
                .get(&SampleType::EchoSubscription)
                .unwrap()
                .len(),
            0
        );

        assert_eq!(
            sampler
                .view
                .get(&SampleType::ReadySubscription)
                .unwrap()
                .len(),
            0
        );

        assert_eq!(
            sampler
                .view
                .get(&SampleType::DeliverySubscription)
                .unwrap()
                .len(),
            0
        );
    }

    #[tokio::test]
    async fn confirming_echo_peer_and_create_view() {
        cyclerng::utils::set_cycle([1]);

        let (_, cmd_receiver) = mpsc::channel(10);
        let (event_sender, mut event_receiver) = broadcast::channel(10);
        let (view_sender, mut view_receiver) = mpsc::channel(10);

        let g = |a, b| ((a as f32) * b) as usize;
        let mut sampler = Sampler::new(
            ReliableBroadcastParams {
                echo_threshold: g(1, 0.66),
                echo_sample_size: 1,
                ready_threshold: g(1, 0.33),
                ready_sample_size: 1,
                delivery_threshold: g(1, 0.66),
                delivery_sample_size: 1,
            },
            cmd_receiver,
            event_sender,
            view_sender,
        );

        sampler.pending_echo_subscribtions.insert("peer_1".into());
        sampler
            .handle_peer_confirmation(SampleType::EchoSubscription, "peer_1".into())
            .await
            .expect("Peer confirmation failed");

        assert_eq!(sampler.pending_echo_subscribtions.len(), 0);
        assert!(matches!(
            event_receiver.try_recv(),
            Err(TryRecvError::Empty)
        ));
        assert!(matches!(
            view_receiver.try_recv(),
            Err(tokio::sync::mpsc::error::TryRecvError::Empty)
        ));

        sampler.pending_subs_state_change_follow_up().await;
        assert!(matches!(
            view_receiver.try_recv(),
            Err(tokio::sync::mpsc::error::TryRecvError::Empty)
        ));

        sampler.status = SampleProviderStatus::BuildingNewView;
        sampler.pending_subs_state_change_follow_up().await;

        let mut expected = SampleView::new();
        let mut hash_set = HashSet::new();
        hash_set.insert("peer_1".to_string());
        expected.insert(SampleType::EchoSubscription, hash_set);

        assert!(matches!(view_receiver.try_recv(), Ok(produced_view) if produced_view == expected));
    }
}
