mod cyclerng;
mod sampling;

// Move to transport crate
pub type Peer = String;
use std::{cmp::min, collections::HashSet};

use tce_transport::{ProtocolEvents, ReliableBroadcastParams};
use tokio::sync::{broadcast, mpsc, oneshot};
use topos_p2p::PeerId;
use tracing::{debug, error, info, warn};

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

#[derive(Debug, Clone, Eq, PartialEq, Default)]
pub struct SubscribersView {
    pub echo: HashSet<PeerId>,
    pub ready: HashSet<PeerId>,
}

impl SubscribersView {
    pub fn get_subscribers(&self) -> Vec<PeerId> {
        self.echo
            .iter()
            .chain(self.ready.iter())
            .cloned()
            .collect::<HashSet<_>>()
            .into_iter()
            .collect::<Vec<_>>()
    }
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub enum SubscribersUpdate {
    NewEchoSubscriber(PeerId),
    RemoveEchoSubscriber(PeerId),
    RemoveEchoSubscribers(HashSet<PeerId>),
    NewReadySubscriber(PeerId),
    RemoveReadySubscriber(PeerId),
    RemoveReadySubscribers(HashSet<PeerId>),
}

// Same set, merge
// Need to seperate echo and ready (echo removes it from echo set, ready removes it from ready set)
// HashSet is all participants, once I receive echo | ready | delivery, I remove it to get to the threshold
// Maybe structure for keeping track of different counters
#[derive(Debug, Clone, Eq, PartialEq, Default)]
pub struct SubscriptionsView {
    // pub nodes: HashSet<PeerId>,

    // All have the same peers (whole network) initially
    pub echo: HashSet<PeerId>,
    pub ready: HashSet<PeerId>,
    pub delivery: HashSet<PeerId>,
}

impl SubscriptionsView {
    pub fn is_some(&self) -> bool {
        !self.is_none()
    }

    pub fn is_none(&self) -> bool {
        self.echo.is_empty() && self.ready.is_empty() && self.delivery.is_empty()
    }

    pub fn get_subscriptions(&self) -> Vec<PeerId> {
        self.echo
            .iter()
            // .chain(self.ready.iter())
            // .chain(self.delivery.iter())
            // .cloned()
            // .collect::<HashSet<_>>()
            // .into_iter()
            .collect()
    }
}

pub struct Sampler {
    params: ReliableBroadcastParams,
    command_receiver: mpsc::Receiver<SamplerCommand>,
    event_sender: broadcast::Sender<ProtocolEvents>,
    visible_peers: Vec<PeerId>,

    pending_subscriptions: SubscriptionsView,
    subscriptions: SubscriptionsView,
    subscribers: SubscribersView, // View of the peers that are following me. Kept for purpose of sending RemoveSubscriber to DoubleEcho
    subscriptions_sender: mpsc::Sender<SubscriptionsView>,
    subscribers_update_sender: mpsc::Sender<SubscribersUpdate>,
    status: SampleProviderStatus,
    pub(crate) shutdown: mpsc::Receiver<oneshot::Sender<()>>,
}

impl Sampler {
    pub fn new(
        params: ReliableBroadcastParams,
        command_receiver: mpsc::Receiver<SamplerCommand>,
        event_sender: broadcast::Sender<ProtocolEvents>,
        subscriptions_sender: mpsc::Sender<SubscriptionsView>,
        subscribers_update_sender: mpsc::Sender<SubscribersUpdate>,
        shutdown: mpsc::Receiver<oneshot::Sender<()>>,
    ) -> Self {
        Self {
            params,
            command_receiver,
            event_sender,
            visible_peers: Vec::new(),
            pending_subscriptions: Default::default(),
            subscriptions: SubscriptionsView::default(),
            subscribers: SubscribersView::default(),
            subscriptions_sender,
            subscribers_update_sender,
            status: SampleProviderStatus::Stabilized,
            shutdown,
        }
    }

    pub async fn run(mut self) {
        #[cfg(feature = "direct")]
        if self
            .event_sender
            .send(ProtocolEvents::StableSample(vec![]))
            .is_err()
        {
            error!("Unable to notify the TCE runtime for the set of samples");
        }
        let shutdowned: Option<oneshot::Sender<()>> = loop {
            tokio::select! {
                shutdown = self.shutdown.recv() => {
                    debug!("Shutting down sampler, {:?}", shutdown);
                    break shutdown;
                },
                Some(command) = self.command_receiver.recv() => {
                    match command {
                        SamplerCommand::PeerConfirmationFailed { peer, sample_type } => {
                            self.handle_peer_confirmation_failure(sample_type, peer).await;

                            self.pending_subs_state_change_follow_up().await;
                        }
                        SamplerCommand::ConfirmPeer { peer, sample_type, sender } => {
                            let _ = match self.handle_peer_confirmation(sample_type, peer).await {
                                Ok(_) => sender.send(Ok(())),
                                Err(error) => sender.send(Err(error)),
                            };

                            self.pending_subs_state_change_follow_up().await;
                        }
                        SamplerCommand::PeersChanged { peers } => self.peers_changed(peers).await,
                        SamplerCommand::ForceResample => self.reset_samples().await
                    }
                }
                else => break None

            }
        };

        if let Some(sender) = shutdowned {
            info!("Shutting down the sampler...");
            _ = sender.send(());
        } else {
            error!("Shutting down the sampler due to error...");
        }
    }
}

impl Sampler {
    async fn peers_changed(&mut self, peers: Vec<PeerId>) {
        self.visible_peers = peers;
        self.reset_samples().await;
    }

    async fn pending_subs_state_change_follow_up(&mut self) {
        match self.status {
            SampleProviderStatus::Stabilized => {
                // Do nothing
            }
            SampleProviderStatus::BuildingNewView => {
                let stable_view = self.pending_subscriptions.echo.is_empty()
                    && self.pending_subscriptions.ready.is_empty()
                    && self.pending_subscriptions.delivery.is_empty();
                if stable_view {
                    // Attempt to send the new subscription view to the Broadcaster
                    match self
                        .subscriptions_sender
                        .send(self.subscriptions.clone())
                        .await
                    {
                        Ok(_) => {
                            self.status = SampleProviderStatus::Stabilized;
                            info!("Successfully established the new set of samples");
                            if self
                                .event_sender
                                .send(ProtocolEvents::StableSample(
                                    self.subscriptions.get_subscriptions(),
                                ))
                                .is_err()
                            {
                                error!("Unable to notify the TCE runtime for the set of samples");
                            }
                        }
                        Err(e) => {
                            error!("Failed to send new subscription sample view {:?} ", e);
                        }
                    }
                }
            }
        }
    }

    async fn handle_peer_confirmation_failure(&mut self, sample_type: SampleType, peer: PeerId) {
        warn!("Failed to confirm the Peer {peer} for the sample {sample_type:?}");

        match sample_type {
            SampleType::EchoSubscription => self.pending_subscriptions.echo.remove(&peer),
            SampleType::ReadySubscription => self.pending_subscriptions.ready.remove(&peer),
            SampleType::DeliverySubscription => self.pending_subscriptions.delivery.remove(&peer),
            _ => true,
        };
    }

    async fn handle_peer_confirmation(
        &mut self,
        sample_type: SampleType,
        peer: PeerId,
    ) -> Result<bool, ()> {
        info!("Successful handshake with the Peer {peer} for the sample {sample_type:?}",);
        let inserted = match sample_type {
            SampleType::EchoSubscription => {
                if self.pending_subscriptions.echo.remove(&peer) {
                    self.subscriptions.echo.insert(peer)
                } else {
                    false
                }
            }
            SampleType::ReadySubscription => {
                if self.pending_subscriptions.ready.remove(&peer) {
                    self.subscriptions.ready.insert(peer)
                } else {
                    false
                }
            }
            SampleType::DeliverySubscription => {
                if self.pending_subscriptions.delivery.remove(&peer) {
                    self.subscriptions.delivery.insert(peer)
                } else {
                    false
                }
            }

            SampleType::EchoSubscriber => {
                let inserted = self.subscribers.echo.insert(peer);
                if let Err(error) = self
                    .subscribers_update_sender
                    .send(SubscribersUpdate::NewEchoSubscriber(peer))
                    .await
                {
                    error!("Unable to send NewEchoSubscriber message {:?}", error);
                    return Err(());
                }
                inserted
            }
            SampleType::ReadySubscriber => {
                let inserted = self.subscribers.ready.insert(peer);
                if let Err(error) = self
                    .subscribers_update_sender
                    .send(SubscribersUpdate::NewReadySubscriber(peer))
                    .await
                {
                    error!("Unable to send NewReadySubscriber message {:?}", error);
                    return Err(());
                }
                inserted
            }
        };
        Ok(inserted)
    }

    async fn reset_samples(&mut self) {
        self.status = SampleProviderStatus::BuildingNewView;

        // Reset invisible echo subscribers
        let (echo_peers_to_keep, echo_peers_to_remove) = self
            .subscribers
            .echo
            .drain()
            .partition(|p| self.visible_peers.contains(p));

        self.subscribers.echo = echo_peers_to_keep;

        // Generate remove echo subscriber event
        if let Err(error) = self
            .subscribers_update_sender
            .send(SubscribersUpdate::RemoveEchoSubscribers(
                echo_peers_to_remove,
            ))
            .await
        {
            error!("Unable to send RemoveEchoSubscribers event {:?}", error);
        }

        // Reset invisible ready subscribers
        let (ready_peers_to_keep, ready_peers_to_remove) = self
            .subscribers
            .ready
            .drain()
            .partition(|p| self.visible_peers.contains(p));

        self.subscribers.ready = ready_peers_to_keep;

        // Generate remove ready subscriber event
        if let Err(error) = self
            .subscribers_update_sender
            .send(SubscribersUpdate::RemoveReadySubscribers(
                ready_peers_to_remove,
            ))
            .await
        {
            error!("Unable to send RemoveReadySubscribers event {:?}", error);
        }

        // Reset subscriptions
        self.subscriptions.echo.clear();
        self.subscriptions.ready.clear();
        self.subscriptions.delivery.clear();

        self.reset_echo_subscription_sample();
        self.reset_ready_subscription_sample();
        self.reset_delivery_subscription_sample();
    }

    /// subscription echo sampling
    fn reset_echo_subscription_sample(&mut self) {
        self.pending_subscriptions.echo.clear();

        debug!(
                "Start the reset of the the Echo Sample currently composed by: {:?}",
                &self.visible_peers
            );

        for peer in &self.visible_peers.value {
            info!("Adding the Peer {peer} to the pending Echo Subscriptions");
            self.pending_subscriptions.echo.insert(*peer);
        }

        if let Err(error) = self.event_sender.send(ProtocolEvents::EchoSubscribeReq {
            peers: echo_candidates.value,
        }) {
            error!("Unable to send event {:?}", error);
        }
    }


    /// subscription ready sampling
    fn reset_ready_subscription_sample(&mut self) {
        self.pending_subscriptions.ready.clear();

        let ready_sizer = |len| min(len, self.params.ready_sample_size);
        match sample_reduce_from(&self.visible_peers, ready_sizer) {
            Ok(ready_candidates) => {
                debug!(
                    "Start the reset of the the Ready Sample currently composed by: {:?}",
                    ready_candidates
                );

                for peer in &ready_candidates.value {
                    info!("Adding the Peer {peer} to the pending Ready Subscriptions");
                    self.pending_subscriptions.ready.insert(*peer);
                }

                if let Err(error) = self.event_sender.send(ProtocolEvents::ReadySubscribeReq {
                    peers: ready_candidates.value,
                }) {
                    error!("Unable to send event {:?}", error);
                }
            }
            Err(e) => {
                error!("Failed to create the sample for the Ready Subscriptions: {e:?}");
            }
        }
    }

    /// subscription delivery sampling
    fn reset_delivery_subscription_sample(&mut self) {
        self.pending_subscriptions.delivery.clear();

        let delivery_sizer = |len| min(len, self.params.delivery_sample_size);
        match sample_reduce_from(&self.visible_peers, delivery_sizer) {
            Ok(delivery_candidates) => {
                info!(
                    "Start the reset of the the Delivery Sample currently composed by: {:?}",
                    delivery_candidates
                );

                for peer in &delivery_candidates.value {
                    info!("Adding the Peer {peer} to the pending Delivery Subscriptions");
                    self.pending_subscriptions.delivery.insert(*peer);
                }

                if let Err(error) = self.event_sender.send(ProtocolEvents::ReadySubscribeReq {
                    peers: delivery_candidates.value,
                }) {
                    error!("Unable to send event {:?}", error);
                }
            }
            Err(e) => {
                error!("Failed to create the sample for the Delivery Subscriptions: {e:?}");
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
    use super::*;
    use rand::{seq::IteratorRandom, Rng};
    use test_log::test;
    use tokio::sync::broadcast::error::TryRecvError;
    use tokio::sync::mpsc::Receiver;

    fn get_sample(peers: &[PeerId], sample_size: usize) -> HashSet<PeerId> {
        let mut rng = rand::thread_rng();
        HashSet::from_iter(peers.iter().cloned().choose_multiple(&mut rng, sample_size))
    }

    fn get_subscriber_view(peers: &[PeerId], sample_size: usize) -> SubscribersView {
        SubscribersView {
            echo: get_sample(peers, sample_size),
            ready: get_sample(peers, sample_size),
        }
    }

    #[test(tokio::test)]
    async fn on_peer_change_subscription_sample_view_is_reset() {
        cyclerng::utils::set_cycle([1]);

        let (_, cmd_receiver) = mpsc::channel(10);
        let (event_sender, mut event_receiver) = broadcast::channel(10);
        let (subscriptions_view_sender, _) = mpsc::channel(10);
        let (subscribers_update_sender, _) = mpsc::channel(10);

        let nb_peers = 100;
        let sample_size = 10;
        let g = |a, b| ((a as f32) * b) as usize;

        let (_sampler_shutdown_sender, sampler_shutdown_receiver) =
            mpsc::channel::<oneshot::Sender<()>>(1);
        let mut sampler = Sampler::new(
            ReliableBroadcastParams {
                echo_threshold: g(sample_size, 0.5),
                echo_sample_size: sample_size,
                ready_threshold: g(sample_size, 0.5),
                ready_sample_size: sample_size,
                delivery_threshold: g(sample_size, 0.5),
                delivery_sample_size: sample_size,
            },
            cmd_receiver,
            event_sender,
            subscriptions_view_sender,
            subscribers_update_sender,
            sampler_shutdown_receiver,
        );

        let mut peers = Vec::new();
        for i in 0..nb_peers {
            let peer = topos_p2p::utils::local_key_pair(Some(i))
                .public()
                .to_peer_id();
            peers.push(peer);
        }

        sampler.peers_changed(peers).await;

        assert!(matches!(
            sampler.status,
            SampleProviderStatus::BuildingNewView
        ));

        assert_eq!(event_receiver.len(), 3);
        assert!(matches!(
            event_receiver.try_recv(),
            Ok(ProtocolEvents::EchoSubscribeReq { peers }) if peers.len() == sampler.params.echo_sample_size
        ));
        assert!(matches!(
            event_receiver.try_recv(),
            Ok(ProtocolEvents::ReadySubscribeReq { peers }) if peers.len() == sampler.params.ready_sample_size
        ));
        assert!(matches!(
            event_receiver.try_recv(),
            Ok(ProtocolEvents::ReadySubscribeReq { peers }) if peers.len() == sampler.params.delivery_sample_size
        ));
        assert!(matches!(
            event_receiver.try_recv(),
            Err(TryRecvError::Empty)
        ));

        assert_eq!(sampler.subscriptions.echo.len(), 0);

        assert_eq!(sampler.subscriptions.ready.len(), 0);

        assert_eq!(sampler.subscriptions.delivery.len(), 0);
    }

    #[test(tokio::test)]
    async fn on_peer_change_subscribers_sample_view_is_updated() {
        cyclerng::utils::set_cycle([1]);

        let (_, cmd_receiver) = mpsc::channel(10);
        let (event_sender, mut _event_receiver) = broadcast::channel(10);
        let (subscriptions_view_sender, _) = mpsc::channel(10);
        let (subscribers_update_sender, _) = mpsc::channel(10);

        let nb_peers = 100;
        let subscription_sample_size = 10;
        let subscriber_sample_size = 8;
        let g = |a, b| ((a as f32) * b) as usize;
        let (_sampler_shutdown_sender, sampler_shutdown_receiver) =
            mpsc::channel::<oneshot::Sender<()>>(1);
        let mut sampler = Sampler::new(
            ReliableBroadcastParams {
                echo_threshold: g(subscription_sample_size, 0.5),
                echo_sample_size: subscription_sample_size,
                ready_threshold: g(subscription_sample_size, 0.5),
                ready_sample_size: subscription_sample_size,
                delivery_threshold: g(subscription_sample_size, 0.5),
                delivery_sample_size: subscription_sample_size,
            },
            cmd_receiver,
            event_sender,
            subscriptions_view_sender,
            subscribers_update_sender,
            sampler_shutdown_receiver,
        );

        let mut peers = Vec::new();
        for i in 0..nb_peers {
            let peer = topos_p2p::utils::local_key_pair(Some(i))
                .public()
                .to_peer_id();
            peers.push(peer);
        }

        let mut subscribers_view = get_subscriber_view(&peers, subscriber_sample_size);
        sampler.subscribers = subscribers_view.clone();

        // Change the peer pool
        let mut rng = rand::thread_rng();
        let (new_echo_subscribers, removed_echo_subscribers): (HashSet<_>, _) = subscribers_view
            .echo
            .drain()
            .partition(|_p| rng.gen_range(0..20) > 10);

        let (new_ready_subscribers, removed_ready_subscribers): (HashSet<_>, _) = subscribers_view
            .ready
            .drain()
            .partition(|_p| rng.gen_range(0..20) > 10);

        // Remove from peers
        let new_peers = peers
            .into_iter()
            .filter(|p| {
                !(removed_echo_subscribers.contains(p) || removed_ready_subscribers.contains(p))
            })
            .collect();

        let expected_echo_subscribers: HashSet<_> = new_echo_subscribers
            .into_iter()
            .filter(|p| !removed_ready_subscribers.contains(p))
            .collect();

        let expected_ready_subscribers: HashSet<_> = new_ready_subscribers
            .into_iter()
            .filter(|p| !removed_echo_subscribers.contains(p))
            .collect();

        sampler.peers_changed(new_peers).await;

        assert_eq!(
            sampler.subscribers.echo.len(),
            expected_echo_subscribers.len()
        );
        assert_eq!(
            sampler.subscribers.ready.len(),
            expected_ready_subscribers.len()
        );
        assert_eq!(sampler.subscribers.echo, expected_echo_subscribers);
        assert_eq!(sampler.subscribers.ready, expected_ready_subscribers);
    }

    #[test(tokio::test)]
    async fn confirming_peers_and_create_expected_subscriptions_view() {
        cyclerng::utils::set_cycle([1]);

        let (_, cmd_receiver) = mpsc::channel(100);
        let (event_sender, mut _event_receiver) = broadcast::channel(100);
        let (subscriptions_view_sender, mut subscriptions_view_receiver) = mpsc::channel(10);
        let (subscribers_update_sender, mut _subscribers_update_receiver) = mpsc::channel(10);

        let g = |a, b| ((a as f32) * b) as usize;
        let nb_peers = 100;
        let subscription_sample_size = 10;
        let (_sampler_shutdown_sender, sampler_shutdown_receiver) =
            mpsc::channel::<oneshot::Sender<()>>(1);
        let mut sampler = Sampler::new(
            ReliableBroadcastParams {
                echo_threshold: g(subscription_sample_size, 0.5),
                echo_sample_size: subscription_sample_size,
                ready_threshold: g(subscription_sample_size, 0.5),
                ready_sample_size: subscription_sample_size,
                delivery_threshold: g(subscription_sample_size, 0.5),
                delivery_sample_size: subscription_sample_size,
            },
            cmd_receiver,
            event_sender,
            subscriptions_view_sender,
            subscribers_update_sender,
            sampler_shutdown_receiver,
        );

        let mut peers = Vec::new();
        for i in 0..nb_peers {
            let peer = topos_p2p::utils::local_key_pair(Some(i))
                .public()
                .to_peer_id();
            peers.push(peer);
        }

        sampler.peers_changed(peers).await;

        let expected_subscriptions_view = sampler.pending_subscriptions.clone();

        for p in sampler.pending_subscriptions.echo.clone() {
            sampler
                .handle_peer_confirmation(SampleType::EchoSubscription, p)
                .await
                .expect("Handle peer confirmation");
        }

        for p in sampler.pending_subscriptions.ready.clone() {
            sampler
                .handle_peer_confirmation(SampleType::ReadySubscription, p)
                .await
                .expect("Handle peer confirmation");
        }

        for p in sampler.pending_subscriptions.delivery.clone() {
            sampler
                .handle_peer_confirmation(SampleType::DeliverySubscription, p)
                .await
                .expect("Handle peer confirmation");
        }

        assert!(sampler.pending_subscriptions.echo.is_empty());
        assert!(sampler.pending_subscriptions.ready.is_empty());
        assert!(sampler.pending_subscriptions.delivery.is_empty());

        sampler.pending_subs_state_change_follow_up().await;

        assert!(
            matches!(subscriptions_view_receiver.try_recv(), Ok(produced_view) if produced_view == expected_subscriptions_view)
        );
    }

    fn handle_subscriber_update(
        subscribers_update_receiver: &mut Receiver<SubscribersUpdate>,
        resulting_echo_subscribers: HashSet<PeerId>,
        resulting_ready_subscribers: HashSet<PeerId>,
    ) -> (HashSet<PeerId>, HashSet<PeerId>) {
        let mut resulting_echo_subscribers = resulting_echo_subscribers;
        let mut resulting_ready_subscribers = resulting_ready_subscribers;
        while let Ok(update) = subscribers_update_receiver.try_recv() {
            match update {
                SubscribersUpdate::NewEchoSubscriber(subscriber) => {
                    resulting_echo_subscribers.insert(subscriber);
                }
                SubscribersUpdate::NewReadySubscriber(subscriber) => {
                    resulting_ready_subscribers.insert(subscriber);
                }
                SubscribersUpdate::RemoveEchoSubscriber(subscriber) => {
                    resulting_echo_subscribers.remove(&subscriber);
                }
                SubscribersUpdate::RemoveReadySubscriber(subscriber) => {
                    resulting_ready_subscribers.remove(&subscriber);
                }
                SubscribersUpdate::RemoveEchoSubscribers(subscribers) => {
                    resulting_echo_subscribers.retain(|p| !subscribers.contains(p));
                }
                SubscribersUpdate::RemoveReadySubscribers(subscribers) => {
                    resulting_ready_subscribers.retain(|p| !subscribers.contains(p));
                }
            }
        }
        (resulting_echo_subscribers, resulting_ready_subscribers)
    }

    #[test(tokio::test)]
    async fn confirming_peers_and_create_expected_subscribers_view() {
        cyclerng::utils::set_cycle([1]);

        let (_, cmd_receiver) = mpsc::channel(100);
        let (event_sender, mut _event_receiver) = broadcast::channel(100);
        let (subscriptions_view_sender, mut _subscriptions_view_receiver) = mpsc::channel(10);
        let (subscribers_update_sender, mut subscribers_update_receiver) = mpsc::channel(100);

        let g = |a, b| ((a as f32) * b) as usize;
        let nb_peers = 100;
        let subscription_sample_size = 10;
        let subscribers_sample_size = 10;
        let (_sampler_shutdown_sender, sampler_shutdown_receiver) =
            mpsc::channel::<oneshot::Sender<()>>(1);
        let mut sampler = Sampler::new(
            ReliableBroadcastParams {
                echo_threshold: g(subscription_sample_size, 0.5),
                echo_sample_size: subscription_sample_size,
                ready_threshold: g(subscription_sample_size, 0.5),
                ready_sample_size: subscription_sample_size,
                delivery_threshold: g(subscription_sample_size, 0.5),
                delivery_sample_size: subscription_sample_size,
            },
            cmd_receiver,
            event_sender,
            subscriptions_view_sender,
            subscribers_update_sender,
            sampler_shutdown_receiver,
        );

        let mut peers = Vec::new();
        for i in 0..nb_peers {
            let peer = topos_p2p::utils::local_key_pair(Some(i))
                .public()
                .to_peer_id();
            peers.push(peer);
        }

        sampler.peers_changed(peers.clone()).await;

        let initial_subscribers_view = get_subscriber_view(&peers, subscribers_sample_size);
        for p in initial_subscribers_view.echo.clone() {
            sampler
                .handle_peer_confirmation(SampleType::EchoSubscriber, p)
                .await
                .expect("Handle peer confirmation");
        }

        for p in initial_subscribers_view.ready.clone() {
            sampler
                .handle_peer_confirmation(SampleType::ReadySubscriber, p)
                .await
                .expect("Handle peer confirmation");
        }

        sampler.pending_subs_state_change_follow_up().await;

        let resulting_echo_subscribers = HashSet::new();
        let resulting_ready_subscribers = HashSet::new();
        let (resulting_echo_subscribers, resulting_ready_subscribers) = handle_subscriber_update(
            &mut subscribers_update_receiver,
            resulting_echo_subscribers,
            resulting_ready_subscribers,
        );

        assert_eq!(sampler.subscribers.echo, resulting_echo_subscribers);
        assert_eq!(sampler.subscribers.ready, resulting_ready_subscribers);

        //Remove some and add additional peers
        let mut rng = rand::thread_rng();
        let (mut new_peers, _removed_peers): (Vec<_>, _) =
            peers.into_iter().partition(|_p| rng.gen_range(0..20) > 10);

        for i in nb_peers..nb_peers + 20 {
            let peer = topos_p2p::utils::local_key_pair(Some(i))
                .public()
                .to_peer_id();
            new_peers.push(peer);
        }
        // Cleanup subscribers according to new peers
        sampler.peers_changed(new_peers.clone()).await;

        // Add some more subscribers
        let additional_subscribers_view = get_subscriber_view(&new_peers, subscribers_sample_size);
        for p in additional_subscribers_view.echo.clone() {
            sampler
                .handle_peer_confirmation(SampleType::EchoSubscriber, p)
                .await
                .unwrap_or_default();
        }

        for p in additional_subscribers_view.ready.clone() {
            sampler
                .handle_peer_confirmation(SampleType::ReadySubscriber, p)
                .await
                .unwrap_or_default();
        }

        let (resulting_echo_subscribers, resulting_ready_subscribers) = handle_subscriber_update(
            &mut subscribers_update_receiver,
            resulting_echo_subscribers,
            resulting_ready_subscribers,
        );

        assert_eq!(sampler.subscribers.echo, resulting_echo_subscribers);
        assert_eq!(sampler.subscribers.ready, resulting_ready_subscribers);
    }
}
