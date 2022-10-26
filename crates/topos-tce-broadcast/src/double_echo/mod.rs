use crate::sampler::SubscribersView;
use crate::{
    sampler::SampleType, trb_store::TrbStore, DoubleEchoCommand, Peer, SubscribersUpdate,
    SubscriptionsView,
};
use std::{
    collections::{HashMap, HashSet, VecDeque},
    time,
};
use tce_transport::{ReliableBroadcastParams, TrbpEvents};
use tokio::sync::{broadcast, mpsc};
use topos_core::uci::{Certificate, CertificateId, DigestCompressed};
use tracing::{debug, error, info, warn};

/// Processing data associated to a Certificate candidate for delivery
/// Sample repartition, one peer may belongs to multiple samples
/// Ready is always sent to current subscribers view
struct DeliveryState {
    pub subscriptions: SubscriptionsView,
}

pub struct DoubleEcho {
    my_peer_id: String,
    params: ReliableBroadcastParams,
    command_receiver: mpsc::Receiver<DoubleEchoCommand>,
    subscriptions_view_receiver: mpsc::Receiver<SubscriptionsView>,
    subscribers_update_receiver: mpsc::Receiver<SubscribersUpdate>,
    event_sender: broadcast::Sender<TrbpEvents>,

    store: Box<dyn TrbStore + Send>,

    cert_candidate: HashMap<Certificate, DeliveryState>,
    pending_delivery: HashSet<Certificate>,
    all_known_certs: Vec<Certificate>,
    delivery_time: HashMap<CertificateId, (time::SystemTime, time::Duration)>,
    subscriptions: SubscriptionsView, // My subscriptions for echo, ready and delivery feedback
    subscribers: SubscribersView,     // Echo and ready subscribers that are following me
    buffer: VecDeque<Certificate>,
}

impl DoubleEcho {
    pub const MAX_BUFFER_SIZE: usize = 2048;

    pub fn new(
        my_peer_id: String,
        params: ReliableBroadcastParams,
        command_receiver: mpsc::Receiver<DoubleEchoCommand>,
        subscriptions_view_receiver: mpsc::Receiver<SubscriptionsView>,
        subscribers_update_receiver: mpsc::Receiver<SubscribersUpdate>,
        event_sender: broadcast::Sender<TrbpEvents>,
        store: Box<dyn TrbStore + Send>,
    ) -> Self {
        Self {
            my_peer_id,
            params,
            command_receiver,
            subscriptions_view_receiver,
            subscribers_update_receiver,
            event_sender,
            store,
            cert_candidate: Default::default(),
            pending_delivery: Default::default(),
            all_known_certs: Default::default(),
            delivery_time: Default::default(),
            subscriptions: SubscriptionsView::default(),
            subscribers: SubscribersView::default(),
            buffer: VecDeque::new(),
        }
    }

    pub(crate) async fn run(mut self) {
        info!("DoubleEcho started");
        loop {
            tokio::select! {
                Some(command) = self.command_receiver.recv() => {
                    match command {
                        DoubleEchoCommand::DeliveredCerts { subnet_id, limit, sender } => {
                            debug!("peer_id: {} DoubleEchoCommand::DeliveredCerts, subnet_id: {}, limit: {}", self.my_peer_id, &subnet_id, &limit);
                            let value = self
                                .store
                                .recent_certificates_for_subnet(&subnet_id, limit)
                                .iter()
                                .filter_map(|cert_id| self.store.cert_by_id(cert_id).ok())
                                .collect();

                            // TODO: Catch send failure
                            let _ = sender.send(Ok(value));
                        }

                        DoubleEchoCommand::BroadcastMany { certificates } if self.buffer.is_empty() => {
                            debug!("peer_id: {} DoubleEchoCommand::BroadcastMany cert ids: {:?}", self.my_peer_id,
                                certificates.iter().map(|cert| &cert.cert_id).collect::<Vec<&CertificateId>>());
                            self.buffer = certificates.into();
                        }

                        DoubleEchoCommand::Broadcast { cert } => {
                            debug!("peer_id: {} DoubleEchoCommand::Broadcast cert_id: {}", self.my_peer_id, &cert.cert_id);
                            if self.buffer.len() < Self::MAX_BUFFER_SIZE {
                                self.buffer.push_back(cert);
                            }
                        }

                        command if self.subscriptions.is_some() => {
                            match command {
                                DoubleEchoCommand::Echo { from_peer, cert } => {
                                    debug!("peer_id: {} handling DoubleEchoCommand::Echo from_peer: {} cert_id: {}", self.my_peer_id, &from_peer, &cert.cert_id);
                                    self.handle_echo(from_peer, cert)},
                                DoubleEchoCommand::Ready { from_peer, cert } => {
                                    debug!("peer_id: {} handling DoubleEchoCommand::Ready from_peer: {} cert_id: {}", self.my_peer_id, &from_peer, &cert.cert_id);
                                    self.handle_ready(from_peer, cert)},
                                DoubleEchoCommand::Deliver { cert, digest } => {
                                    debug!("peer_id: {} handling DoubleEchoCommand::Deliver cert_id: {} digest: {:?}", self.my_peer_id, &cert.cert_id, &digest);
                                    self.handle_deliver(cert, digest)},
                                _ => {}
                            }

                            self.state_change_follow_up();
                        }
                        _ => {}
                    }
                }

                Some(new_subscriptions_view) = self.subscriptions_view_receiver.recv() => {
                    info!("peer_id {} new sample view received {:?}", &self.my_peer_id, &new_subscriptions_view);
                    self.subscriptions = new_subscriptions_view;
                }

                Some(new_subscribers_update) = self.subscribers_update_receiver.recv() => {
                    info!("peer_id {} new subscribers update {:?}", &self.my_peer_id, &new_subscribers_update);
                    match new_subscribers_update {
                        SubscribersUpdate::NewEchoSubscriber(peer) if !self.subscriptions.echo.contains(&peer) => {
                            self.subscribers.echo.insert(peer);
                        }
                        SubscribersUpdate::NewReadySubscriber(peer) if !self.subscriptions.ready.contains(&peer) => {
                            self.subscribers.ready.insert(peer);
                        }
                        SubscribersUpdate::RemoveEchoSubscriber(peer) => {
                            self.subscribers.echo.remove(&peer);
                        }
                        SubscribersUpdate::RemoveEchoSubscribers(peers) => {
                            self.subscribers.echo = self.subscribers.echo.drain().filter(|v| !peers.contains(v.as_str())).collect();
                        }
                        SubscribersUpdate::RemoveReadySubscriber(peer) => {
                            self.subscribers.ready.remove(&peer);
                        }
                        SubscribersUpdate::RemoveReadySubscribers(peers) => {
                            self.subscribers.ready = self.subscribers.ready.drain().filter(|v| !peers.contains(v.as_str())).collect();
                        }
                        _ => {}
                    }
                }
            }

            // Broadcast next certificate
            if self.subscriptions.is_some() {
                while let Some(cert) = self.buffer.pop_front() {
                    self.handle_broadcast(cert);
                }
            }
        }
    }
}

impl DoubleEcho {
    fn sample_consume_peer(from_peer: &str, state: &mut DeliveryState, sample_type: SampleType) {
        match sample_type {
            SampleType::EchoSubscription => state.subscriptions.echo.remove(from_peer),
            SampleType::ReadySubscription => state.subscriptions.ready.remove(from_peer),
            SampleType::DeliverySubscription => state.subscriptions.delivery.remove(from_peer),
            _ => false,
        };
    }

    fn handle_echo(&mut self, from_peer: Peer, cert: Certificate) {
        if let Some(state) = self.cert_candidate.get_mut(&cert) {
            Self::sample_consume_peer(&from_peer, state, SampleType::EchoSubscription);
        }
    }

    fn handle_ready(&mut self, from_peer: Peer, cert: Certificate) {
        if let Some(state) = self.cert_candidate.get_mut(&cert) {
            Self::sample_consume_peer(&from_peer, state, SampleType::ReadySubscription);
            Self::sample_consume_peer(&from_peer, state, SampleType::DeliverySubscription);
        }
    }

    fn handle_broadcast(&mut self, cert: Certificate) {
        info!(
            "peer_id: {} handling broadcast of cert_id {}",
            &self.my_peer_id, &cert.cert_id
        );
        let digest = self
            .store
            .flush_digest_view(&cert.initial_subnet_id)
            .unwrap_or_default();

        self.dispatch(cert, digest);
    }

    fn handle_deliver(&mut self, cert: Certificate, digest: DigestCompressed) {
        self.dispatch(cert, digest)
    }

    /// Called to process potentially new certificate:
    /// - either submitted from API ( [tce_transport::TrbpCommands::Broadcast] command)
    /// - or received through the gossip (first step of protocol exchange)
    fn dispatch(&mut self, cert: Certificate, digest: DigestCompressed) {
        if self.cert_pre_delivery_check(&cert).is_err() {
            error!("Error on the pre cert delivery check");
            return;
        }
        // Don't gossip one cert already gossiped
        if self.cert_candidate.contains_key(&cert) {
            return;
        }

        if self.store.cert_by_id(&cert.cert_id).is_ok() {
            return;
        }

        // Gossip the certificate to all my peers
        let gossip_peers = self.gossip_peers();
        debug!(
            "peer_id: {} dispatching gossip event cert_id: {} to gossip peers {:?}",
            &self.my_peer_id, cert.cert_id, &gossip_peers
        );
        let _ = self.event_sender.send(TrbpEvents::Gossip {
            peers: gossip_peers, // considered as the G-set for erdos-renyi
            cert: cert.clone(),
            digest: digest.clone(),
        });

        // Trigger event of new certificate candidate for delivery
        self.start_delivery(cert, digest);
    }

    /// Make gossip peer list from echo and ready
    /// subscribers that listen to me
    fn gossip_peers(&self) -> Vec<Peer> {
        self.subscriptions
            .get_subscriptions()
            .into_iter()
            .chain(self.subscribers.get_subscribers().into_iter())
            .collect::<HashSet<_>>() //Filter duplicates
            .into_iter()
            .collect::<Vec<Peer>>()
    }

    fn start_delivery(&mut self, cert: Certificate, digest: DigestCompressed) {
        debug!(
            "🙌 StartDelivery[{:?}]\t Peer:{:?}",
            &cert.cert_id, &self.my_peer_id
        );
        // Add new entry for the new Cert candidate
        match self.delivery_state_for_new_cert() {
            Some(delivery_state) => {
                self.cert_candidate.insert(cert.clone(), delivery_state);
            }
            None => {
                error!("[{:?}] Ill-formed samples", self.my_peer_id);
                let _ = self.event_sender.send(TrbpEvents::Die);
                return;
            }
        }
        self.all_known_certs.push(cert.clone());
        self.store.new_cert_candidate(&cert, &digest);
        self.delivery_time.insert(
            cert.cert_id.clone(),
            (time::SystemTime::now(), Default::default()),
        );
        // Send Echo to the echo sample
        let echo_peers = self.subscribers.echo.iter().cloned().collect::<Vec<_>>();
        if echo_peers.is_empty() {
            warn!("[{:?}] EchoSubscriber peers set is empty", self.my_peer_id);
            return;
        }

        let _ = self.event_sender.send(TrbpEvents::Echo {
            peers: echo_peers,
            cert,
        });
    }

    /// Build initial delivery state
    fn delivery_state_for_new_cert(&mut self) -> Option<DeliveryState> {
        let subscriptions = self.subscriptions.clone();
        // check inbound sets are not empty
        if subscriptions.echo.is_empty()
            || subscriptions.ready.is_empty()
            || subscriptions.delivery.is_empty()
        {
            None
        } else {
            Some(DeliveryState { subscriptions })
        }
    }

    fn state_change_follow_up(&mut self) {
        let mut state_modified = false;
        let mut gen_evts = Vec::<TrbpEvents>::new();
        // For all current Cert on processing
        for (cert, state_to_delivery) in &mut self.cert_candidate {
            if !state_to_delivery.subscriptions.ready.is_empty()
                && (is_e_ready(&self.params, state_to_delivery)
                    || is_r_ready(&self.params, state_to_delivery))
            {
                // Fanout the Ready messages to my subscribers
                let readies = self.subscribers.ready.iter().cloned().collect::<Vec<_>>();
                if !readies.is_empty() {
                    gen_evts.push(TrbpEvents::Ready {
                        peers: readies.clone(),
                        cert: cert.clone(),
                    });
                }
                state_to_delivery.subscriptions.ready.clear();
            }

            if is_ok_to_deliver(&self.params, state_to_delivery) {
                self.pending_delivery.insert(cert.clone());
                state_modified = true;
            }
        }

        if state_modified {
            self.cert_candidate
                .retain(|c, _| !self.pending_delivery.contains(c));

            let cert_to_pending = self
                .pending_delivery
                .iter()
                .cloned()
                .filter(|c| self.cert_post_delivery_check(c).is_ok())
                .collect::<Vec<_>>();

            for cert in &cert_to_pending {
                if self.store.apply_cert(cert).is_ok() {
                    let mut d = time::Duration::from_millis(0);
                    if let Some((from, duration)) = self.delivery_time.get_mut(&cert.cert_id) {
                        *duration = from.elapsed().unwrap();
                        d = *duration;

                        tce_telemetry::span_cert_delivery(
                            self.my_peer_id.clone(),
                            &cert.cert_id,
                            *from,
                            time::SystemTime::now(),
                            Default::default(),
                        )
                    }
                    self.pending_delivery.remove(cert);
                    debug!(
                        "📝 Accepted[{:?}]\t Peer:{:?}\t Delivery time: {:?}",
                        &cert.cert_id, self.my_peer_id, d
                    );

                    _ = self.event_sender.send(TrbpEvents::CertificateDelivered {
                        certificate: cert.clone(),
                    });
                }
            }
        }

        for evt in gen_evts {
            let _ = self.event_sender.send(evt);
        }
    }

    /// Here comes test that can be done before delivery process
    /// in order to avoid going to delivery process for a Cert
    /// that is already known as incorrect
    fn cert_pre_delivery_check(&self, cert: &Certificate) -> Result<(), ()> {
        if cert.check_signature().is_err() {
            error!("Error on the signature");
        }

        if cert.check_proof().is_err() {
            error!("Error on the proof");
        }

        Ok(())
    }

    /// Here comes test that is necessarily done after delivery
    fn cert_post_delivery_check(&self, cert: &Certificate) -> Result<(), ()> {
        if self.store.check_precedence(cert).is_err() {
            warn!("Precedence not yet satisfied {:?}", cert);
        }

        if self.store.check_digest_inclusion(cert).is_err() {
            warn!("Inclusion check not yet satisfied {:?}", cert);
        }
        Ok(())
    }
}

// state checkers
fn is_ok_to_deliver(params: &ReliableBroadcastParams, state: &DeliveryState) -> bool {
    match params
        .delivery_sample_size
        .checked_sub(state.subscriptions.delivery.len())
    {
        Some(consumed) => consumed >= params.delivery_threshold,
        None => false,
    }
}

fn is_e_ready(params: &ReliableBroadcastParams, state: &DeliveryState) -> bool {
    match params
        .echo_sample_size
        .checked_sub(state.subscriptions.echo.len())
    {
        Some(consumed) => consumed >= params.echo_threshold,
        None => false,
    }
}

fn is_r_ready(params: &ReliableBroadcastParams, state: &DeliveryState) -> bool {
    match params
        .ready_sample_size
        .checked_sub(state.subscriptions.ready.len())
    {
        Some(consumed) => consumed >= params.ready_threshold,
        None => false,
    }
}

#[cfg(test)]
mod tests {

    use std::{iter::FromIterator, usize};

    use super::*;
    use crate::mem_store::TrbMemStore;
    // use rand::{distributions::Uniform, Rng};
    use rand::seq::IteratorRandom;
    use tokio::{spawn, sync::broadcast::error::TryRecvError};

    fn get_sample(peers: &[Peer], sample_size: usize) -> HashSet<Peer> {
        let mut rng = rand::thread_rng();
        HashSet::from_iter(peers.iter().cloned().choose_multiple(&mut rng, sample_size))
    }

    fn get_subscriptions_view(peers: &[Peer], sample_size: usize) -> SubscriptionsView {
        let mut expected_view = SubscriptionsView::default();
        expected_view.echo = get_sample(&peers, sample_size);
        expected_view.ready = get_sample(&peers, sample_size);
        expected_view.delivery = get_sample(&peers, sample_size);
        expected_view
    }

    fn get_subscriber_view(peers: &[Peer], sample_size: usize) -> SubscribersView {
        let mut expected_view = SubscribersView::default();
        expected_view.echo = get_sample(&peers, sample_size);
        expected_view.ready = get_sample(&peers, sample_size);
        expected_view
    }

    #[tokio::test]
    async fn handle_receiving_sample_view() {
        let (_subscriptions_view_sender, subscriptions_view_receiver) = mpsc::channel(10);
        let (_subscribers_update_sender, subscribers_update_receiver) = mpsc::channel(10);

        // Network parameters
        let nb_peers = 100;
        let subscription_sample_size = 10;
        let subscriber_sample_size = 8;
        let g = |a, b| ((a as f32) * b) as usize;
        let broadcast_params = ReliableBroadcastParams {
            echo_threshold: g(subscription_sample_size, 0.5),
            echo_sample_size: subscription_sample_size,
            ready_threshold: g(subscription_sample_size, 0.5),
            ready_sample_size: subscription_sample_size,
            delivery_threshold: g(subscription_sample_size, 0.5),
            delivery_sample_size: subscription_sample_size,
        };

        // List of peers
        let mut peers = Vec::new();
        for i in 0..nb_peers {
            peers.push(format!("peer_{i}"));
        }

        let my_peer_id = peers[0].clone();
        let expected_subscriptions_view = get_subscriptions_view(&peers, subscription_sample_size);
        let expected_subscriber_view = get_subscriber_view(&peers, subscriber_sample_size);

        // Double Echo
        let (_cmd_sender, cmd_receiver) = mpsc::channel(10);
        let (event_sender, mut event_receiver) = broadcast::channel(10);
        let mut double_echo = DoubleEcho::new(
            my_peer_id,
            broadcast_params.clone(),
            cmd_receiver,
            subscriptions_view_receiver,
            subscribers_update_receiver,
            event_sender,
            Box::new(TrbMemStore::default()),
        );

        assert!(double_echo.subscriptions.is_none());
        double_echo.subscriptions = expected_subscriptions_view.clone();
        double_echo.subscribers = expected_subscriber_view;

        let le_cert = Certificate::new("0".to_string(), "0".to_string(), vec![]);
        double_echo.handle_broadcast(le_cert.clone());

        assert_eq!(event_receiver.len(), 2);

        assert!(matches!(
            event_receiver.try_recv(),
            Ok(TrbpEvents::Gossip { peers, .. }) if peers.len() == double_echo.gossip_peers().len()
        ));

        assert!(matches!(
            event_receiver.try_recv(),
            Ok(TrbpEvents::Echo { peers, .. }) if peers.len() == subscriber_sample_size));

        assert!(matches!(
            event_receiver.try_recv(),
            Err(TryRecvError::Empty)
        ));

        // Echo phase
        let echo_subscription = expected_subscriptions_view
            .echo
            .into_iter()
            .take(broadcast_params.echo_threshold)
            .collect::<Vec<_>>();

        // Send just enough Echo to reach the threshold
        for p in echo_subscription {
            double_echo.handle_echo(p.clone(), le_cert.clone());
        }

        assert_eq!(event_receiver.len(), 0);

        double_echo.state_change_follow_up();
        assert_eq!(event_receiver.len(), 1);

        assert!(matches!(
            event_receiver.try_recv(),
            Ok(TrbpEvents::Ready { peers, .. }) if peers.len() == subscriber_sample_size
        ));

        // Ready phase
        let delivery_subscription = expected_subscriptions_view
            .delivery
            .into_iter()
            .take(broadcast_params.delivery_threshold)
            .collect::<Vec<_>>();

        // Send just enough Ready to reach the delivery threshold
        for p in delivery_subscription {
            double_echo.handle_ready(p.clone(), le_cert.clone());
        }

        assert_eq!(event_receiver.len(), 0);

        double_echo.state_change_follow_up();
        assert_eq!(event_receiver.len(), 1);

        assert!(matches!(
            event_receiver.try_recv(),
            Ok(TrbpEvents::CertificateDelivered { certificate }) if certificate == le_cert
        ));
    }

    #[tokio::test]
    async fn buffering_certificate() {
        let (subscriptions_view_sender, subscriptions_view_receiver) = mpsc::channel(30);
        let (subscribers_update_sender, subscribers_update_receiver) = mpsc::channel(30);

        // Network parameters
        let nb_peers = 100;
        let subscription_sample_size = 10;
        let subscriber_sample_size = 8;
        let g = |a, b| ((a as f32) * b) as usize;
        let broadcast_params = ReliableBroadcastParams {
            echo_threshold: g(subscription_sample_size, 0.5),
            echo_sample_size: subscription_sample_size,
            ready_threshold: g(subscription_sample_size, 0.5),
            ready_sample_size: subscription_sample_size,
            delivery_threshold: g(subscription_sample_size, 0.5),
            delivery_sample_size: subscription_sample_size,
        };

        // List of peers
        let mut peers = Vec::new();
        for i in 0..nb_peers {
            peers.push(format!("peer_{i}"));
        }

        let my_peer_id = peers[0].clone();
        let expected_subscriptions_view = get_subscriptions_view(&peers, subscription_sample_size);
        let expected_subscriber_view = get_subscriber_view(&peers, subscriber_sample_size);

        // Double Echo
        let (cmd_sender, cmd_receiver) = mpsc::channel(10);
        let (event_sender, mut event_receiver) = broadcast::channel(10);
        let double_echo = DoubleEcho::new(
            my_peer_id,
            broadcast_params.clone(),
            cmd_receiver,
            subscriptions_view_receiver,
            subscribers_update_receiver,
            event_sender,
            Box::new(TrbMemStore::default()),
        );

        spawn(double_echo.run());

        // Add subscribers
        for peer in expected_subscriber_view.echo.clone() {
            subscribers_update_sender
                .send(SubscribersUpdate::NewEchoSubscriber(peer.clone()))
                .await
                .expect("Added new subscriber");
        }
        for peer in expected_subscriber_view.ready.clone() {
            subscribers_update_sender
                .send(SubscribersUpdate::NewReadySubscriber(peer.clone()))
                .await
                .expect("Added new subscriber");
        }
        // Wait to receive subscribers
        tokio::time::sleep(tokio::time::Duration::from_millis(1000)).await;

        let le_cert = Certificate::default();
        cmd_sender
            .send(DoubleEchoCommand::Broadcast {
                cert: le_cert.clone(),
            })
            .await
            .expect("Cannot send broadcast command");

        assert_eq!(event_receiver.len(), 0);
        subscriptions_view_sender
            .send(expected_subscriptions_view.clone())
            .await
            .expect("Cannot send expected view");

        let mut received_gossip_commands: Vec<(HashSet<Peer>, Certificate)> = Vec::new();
        let assertion = async {
            loop {
                while let Ok(event) = event_receiver.try_recv() {
                    if let TrbpEvents::Gossip { peers, cert, .. } = event {
                        received_gossip_commands
                            .push((peers.into_iter().collect::<HashSet<Peer>>(), cert));
                    }
                }
                tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;
            }
        };

        let _ = tokio::time::timeout(std::time::Duration::from_millis(1000), assertion).await;

        debug!(
            "ECHO SUBSCRIBERS: {:#?}\n READY SUBCRIBERS {:#?}\n RECEIVED {:#?}",
            &expected_subscriber_view.echo,
            &expected_subscriber_view.ready,
            received_gossip_commands[0].0
        );
        // Check if gossip Event is sent to all peers
        let mut all_gossip_peers: HashSet<Peer> = expected_subscriptions_view
            .get_subscriptions()
            .into_iter()
            .collect::<HashSet<Peer>>();
        all_gossip_peers.extend(expected_subscriber_view.echo);
        all_gossip_peers.extend(expected_subscriber_view.ready);

        assert_eq!(received_gossip_commands.len(), 1);
        assert_eq!(received_gossip_commands[0].0, all_gossip_peers);
        assert_eq!(received_gossip_commands[0].1.cert_id, le_cert.cert_id);
    }
}
