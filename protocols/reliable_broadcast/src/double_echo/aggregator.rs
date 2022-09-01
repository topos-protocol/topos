//! Protocol implementation guts.
//!
use std::collections::{HashMap, HashSet};
use std::fmt::{Debug, Formatter};
use std::sync::{Arc, Mutex};
use std::time;

#[allow(unused)]
use opentelemetry::KeyValue;
use tokio::sync::broadcast;
use tokio::sync::mpsc;

use tce_transport::{ReliableBroadcastParams, TrbpCommands, TrbpEvents};
use topos_core::uci::{Certificate, CertificateId, DigestCompressed};

use crate::sampler::{SampleType, SampleView};
use crate::{sampler, Peer, TrbInternalCommand};
use crate::{trb_store::TrbStore, ReliableBroadcastConfig};

/// Processing data associated to a Certificate candidate for delivery
/// Sample repartition, one peer may belongs to multiple samples
type DeliveryState = HashMap<SampleType, HashSet<Peer>>;

/// Protocol aggregate
///
/// - samples (peer sets)
/// - cert state data
/// - finite state machine functions
/// - message definitions and data structures
pub struct ReliableBroadcast {
    my_peer_id: Peer,
    pub(crate) broadcast_commands_channel: mpsc::Sender<TrbInternalCommand>,
    pub events_subscribers: Vec<broadcast::Sender<TrbpEvents>>,
    tx_exit: mpsc::UnboundedSender<()>,
    pub(crate) store: Box<dyn TrbStore + Send>,
    params: ReliableBroadcastParams,

    // TODO: Some of the following might need to be moved to `store`
    pub cert_candidate: HashMap<Certificate, DeliveryState>,
    delivered_pending: HashSet<Certificate>,
    pub all_known_certs: Vec<Certificate>,
    pub delivery_time: HashMap<CertificateId, (time::SystemTime, time::Duration)>,
    current_sample_view: Option<SampleView>,
}

impl Debug for ReliableBroadcast {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ReliableBroadcast instance")
            .field("params: ", &self.params)
            .field("cert candidates: ", &self.cert_candidate)
            .finish()
    }
}

fn sample_consume_peer(peer_to_consume: &String, state: &mut DeliveryState, sample: SampleType) {
    if let Some(peers) = state.get_mut(&sample) {
        peers.remove(peer_to_consume);
    }
}

impl ReliableBroadcast {
    pub fn spawn_new(
        config: ReliableBroadcastConfig,
        mut sample_view_receiver: mpsc::Receiver<SampleView>,
    ) -> Arc<Mutex<ReliableBroadcast>> {
        let (b_command_sender, mut b_command_rcv) = mpsc::channel::<TrbInternalCommand>(2048);
        let (tx_exit, mut rx_exit) = mpsc::unbounded_channel::<()>();
        let me = Arc::new(Mutex::from(Self {
            my_peer_id: config.my_peer_id.clone(),
            broadcast_commands_channel: b_command_sender,
            events_subscribers: Vec::new(),
            tx_exit,
            store: config.store,
            params: config.trbp_params.clone(),
            cert_candidate: HashMap::new(),
            delivered_pending: HashSet::new(),
            all_known_certs: Vec::new(),
            delivery_time: HashMap::new(),
            current_sample_view: None,
        }));
        // spawn running closure
        let me_cl = me.clone();
        tokio::spawn(async move {
            loop {
                tokio::select! {
                    // poll commands channel
                    cmd = b_command_rcv.recv(), if Self::has_sample_view(me_cl.clone()) => {
                        Self::on_command(me_cl.clone(), cmd);
                    }
                    // poll new sample view
                    new_sample_view = sample_view_receiver.recv() => {
                        Self::on_new_sample_view(me_cl.clone(), new_sample_view);
                    }
                    // exit command
                    Some(_) = rx_exit.recv() => {
                        break;
                    }
                }
            }
        });
        me
    }

    fn has_sample_view(data: Arc<Mutex<ReliableBroadcast>>) -> bool {
        let aggr = data.lock().unwrap();
        aggr.current_sample_view.is_some()
    }

    fn on_new_sample_view(data: Arc<Mutex<ReliableBroadcast>>, mb_new_view: Option<SampleView>) {
        log::info!("on_new_sample_view({:?})", &mb_new_view);
        let mut aggr = data.lock().unwrap();
        if let Some(new_view) = mb_new_view {
            log::debug!(
                "new_sample_view - [{:?}] New sample view: {:?}",
                aggr.my_peer_id,
                new_view
            );
            aggr.current_sample_view = Some(new_view);
        } else {
            log::warn!("Failure on the sample view channel");
        }
    }

    fn on_command(data: Arc<Mutex<ReliableBroadcast>>, mb_cmd: Option<TrbInternalCommand>) {
        let mut aggr = data.lock().unwrap();

        // Execute
        match mb_cmd {
            Some(TrbInternalCommand::Command(cmd)) => match cmd {
                TrbpCommands::StartUp => {
                    aggr.on_cmd_start_up();
                }
                TrbpCommands::Shutdown => {
                    aggr.on_cmd_shut_down();
                }
                TrbpCommands::OnBroadcast { cert } => {
                    aggr.on_cmd_broadcast(cert);
                }
                TrbpCommands::OnGossip { cert, digest } => {
                    aggr.on_cmd_gossip(cert, digest);
                }
                TrbpCommands::OnEcho { from_peer, cert } => {
                    aggr.on_cmd_on_echo(from_peer, cert);
                }
                TrbpCommands::OnReady { from_peer, cert } => {
                    aggr.on_cmd_on_ready(from_peer, cert);
                }
                _ => {}
            },
            Some(TrbInternalCommand::DeliveredCerts {
                subnet_id,
                limit,
                sender,
            }) => {
                let value = aggr
                    .store
                    .recent_certificates_for_subnet(&subnet_id, limit)
                    .iter()
                    .filter_map(|cert_id| aggr.store.cert_by_id(cert_id).ok())
                    .collect();

                // TODO: Catch send failure
                let _ = sender.send(Ok(value));
            }
            _ => {
                log::warn!("empty command was passed");
            }
        }

        // Follow up
        // bookkeeping of threshold and other state triggering events
        aggr.state_change_follow_up();
    }

    fn send_out_events(&mut self, evt: TrbpEvents) {
        for tx in &self.events_subscribers {
            // FIXME: When error is returned it means that receiving side of the channel is closed
            // Thus we better remove the sender from our subscribers
            let _ = tx.send(evt.clone());
        }
    }

    pub fn on_cmd_start_up(&mut self) {
        self.send_out_events(TrbpEvents::NeedPeers);
    }

    pub fn on_cmd_shut_down(&mut self) {
        log::info!("[{:?}] Shutdown", self.my_peer_id);
        let _ = self.tx_exit.send(());
    }

    /// build initial delivery state
    fn delivery_state_for_new_cert(&mut self) -> Option<DeliveryState> {
        let ds = self.current_sample_view.clone().unwrap();

        // check inbound sets are not empty
        if ds
            .get(&SampleType::EchoSubscription)
            .unwrap_or(&HashSet::<sampler::Peer>::new())
            .is_empty()
            || ds
                .get(&SampleType::ReadySubscription)
                .unwrap_or(&HashSet::<sampler::Peer>::new())
                .is_empty()
            || ds
                .get(&SampleType::DeliverySubscription)
                .unwrap_or(&HashSet::<sampler::Peer>::new())
                .is_empty()
        {
            None
        } else {
            Some(ds)
        }
    }

    /// Called to process potentially new certificate:
    /// - either submitted from API ( [tce_transport::TrbpCommands::Broadcast] command)
    /// - or received through the gossip (first step of protocol exchange)
    fn dispatch(&mut self, cert: Certificate, digest: DigestCompressed) {
        if self.cert_pre_delivery_check(&cert).is_err() {
            log::info!("Error on the pre cert delivery check");
            return;
        }

        // Don't gossip one cert already gossiped
        if self.cert_candidate.contains_key(&cert) {
            return;
        }

        if self.store.cert_by_id(&cert.id).is_ok() {
            return;
        }

        // Gossip the certificate to all my peers

        self.send_out_events(TrbpEvents::Gossip {
            peers: self.gossip_peers(), // considered as the G-set for erdos-renyi
            cert: cert.clone(),
            digest: digest.clone(),
        });
        // Trigger event of new certificate candidate for delivery
        self.start_delivery(cert, digest);
    }

    /// My gossip peers.
    ///
    /// Union of all known peers.
    fn gossip_peers(&self) -> Vec<Peer> {
        if let Some(sample_view_ref) = self.current_sample_view.as_ref() {
            let connected_peers = sample_view_ref
                .get(&SampleType::EchoSubscription)
                .unwrap()
                .iter()
                .chain(
                    sample_view_ref
                        .get(&SampleType::ReadySubscription)
                        .unwrap()
                        .iter(),
                )
                .chain(
                    sample_view_ref
                        .get(&SampleType::DeliverySubscription)
                        .unwrap()
                        .iter(),
                )
                .chain(
                    sample_view_ref
                        .get(&SampleType::EchoSubscriber)
                        .unwrap()
                        .iter(),
                )
                .chain(
                    sample_view_ref
                        .get(&SampleType::ReadySubscriber)
                        .unwrap()
                        .iter(),
                )
                .cloned()
                .collect::<HashSet<_>>()
                .into_iter()
                .collect::<Vec<_>>();

            connected_peers
        } else {
            vec![]
        }
    }

    // Done only by sigma (the sender)
    // in our case, made by the "sequencers"
    // entities between tce and subnet network
    // NOTE: We propagate the digest that we received from elsewhere
    fn on_cmd_gossip(&mut self, cert: Certificate, propagated_digest: DigestCompressed) {
        self.dispatch(cert, propagated_digest);
    }

    // Separated from the Gossip handler for more understandable flow
    // NOTE: We get the digest from the local view
    fn on_cmd_broadcast(&mut self, cert: Certificate) {
        let computed_digest = self
            .store
            .flush_digest_view(&cert.initial_subnet_id)
            .unwrap();
        self.dispatch(cert, computed_digest);
    }

    // pb.Deliver
    fn start_delivery(&mut self, cert: Certificate, digest: DigestCompressed) {
        log::debug!(
            "🙌 StartDelivery[{:?}]\t Peer:{:?}",
            &cert.id,
            &self.my_peer_id
        );
        // Add new entry for the new Cert candidate
        match self.delivery_state_for_new_cert() {
            Some(delivery_state) => {
                self.cert_candidate.insert(cert.clone(), delivery_state);
            }
            None => {
                log::error!("[{:?}] Ill-formed samples", self.my_peer_id);
                self.send_out_events(TrbpEvents::Die);
                return;
            }
        }
        self.all_known_certs.push(cert.clone());
        self.store.new_cert_candidate(&cert, &digest);
        self.delivery_time
            .insert(cert.id, (time::SystemTime::now(), Default::default()));
        // Send Echo to the echo sample
        let sample_to_peers = self.cert_candidate.get(&cert).unwrap();
        let echo_peers = sample_to_peers
            .get(&SampleType::EchoSubscriber)
            .unwrap_or(&HashSet::new())
            .iter()
            .cloned()
            .collect::<Vec<_>>();
        if echo_peers.is_empty() {
            log::warn!("[{:?}] EchoOutbound peers set is empty", self.my_peer_id);
            return;
        }
        self.send_out_events(TrbpEvents::Echo {
            peers: echo_peers,
            cert,
        })
    }

    fn on_cmd_on_echo(&mut self, from_peer: String, cert: Certificate) {
        if let Some(state) = self.cert_candidate.get_mut(&cert) {
            sample_consume_peer(&from_peer, state, SampleType::EchoSubscription);
        }
    }

    fn on_cmd_on_ready(&mut self, from_peer: String, cert: Certificate) {
        if let Some(state) = self.cert_candidate.get_mut(&cert) {
            sample_consume_peer(&from_peer, state, SampleType::ReadySubscription);
            sample_consume_peer(&from_peer, state, SampleType::DeliverySubscription);
        }
    }

    fn state_change_follow_up(&mut self) {
        let mut state_modified = false;
        let mut gen_evts = Vec::<TrbpEvents>::new();
        // For all current Cert on processing
        for (cert, state_to_delivery) in &mut self.cert_candidate {
            if is_e_ready(&self.params, state_to_delivery)
                || is_r_ready(&self.params, state_to_delivery)
            {
                if let Some(ready_sample) = state_to_delivery.get_mut(&SampleType::ReadySubscriber)
                {
                    // Fanout the Ready messages to my subscribers
                    let readies = ready_sample.iter().cloned().collect::<Vec<_>>();
                    if !readies.is_empty() {
                        gen_evts.push(TrbpEvents::Ready {
                            peers: readies.clone(),
                            cert: cert.clone(),
                        });
                    }
                    ready_sample.clear();
                }
            }

            if is_ok_to_deliver(&self.params, state_to_delivery) {
                self.delivered_pending.insert(cert.clone());
                state_modified = true;
            }
        }

        if state_modified {
            self.cert_candidate
                .retain(|c, _| !self.delivered_pending.contains(c));

            let cert_to_pending = self
                .delivered_pending
                .iter()
                .cloned()
                .filter(|c| self.cert_post_delivery_check(c).is_ok())
                .collect::<Vec<_>>();

            for cert in &cert_to_pending {
                if self.store.apply_cert(cert).is_ok() {
                    let mut d = time::Duration::from_millis(0);
                    if let Some((from, duration)) = self.delivery_time.get_mut(&cert.id) {
                        *duration = from.elapsed().unwrap();
                        d = *duration;

                        tce_telemetry::span_cert_delivery(
                            self.my_peer_id.clone(),
                            cert.id,
                            *from,
                            time::SystemTime::now(),
                            Default::default(),
                        )
                    }
                    self.delivered_pending.remove(cert);
                    log::debug!(
                        "📝 Accepted[{:?}]\t Peer:{:?}\t Delivery time: {:?}",
                        &cert.id,
                        self.my_peer_id,
                        d
                    );
                }
            }
        }

        for evt in gen_evts {
            self.send_out_events(evt);
        }
    }

    /// Here comes test that can be done before delivery process
    /// in order to avoid going to delivery process for a Cert
    /// that is already known as incorrect
    #[allow(dead_code)]
    fn cert_pre_delivery_check(&self, cert: &Certificate) -> Result<(), ()> {
        if cert.check_signature().is_err() {
            log::error!("Error on the signature");
        }

        if cert.check_proof().is_err() {
            log::error!("Error on the proof");
        }

        Ok(())
    }

    /// Here comes test that is necessarily done after delivery
    fn cert_post_delivery_check(&self, cert: &Certificate) -> Result<(), ()> {
        if self.store.check_precedence(cert).is_err() {
            log::warn!("Precedence not yet satisfied {:?}", cert);
        }

        if self.store.check_digest_inclusion(cert).is_err() {
            log::warn!("Inclusion check not yet satisfied {:?}", cert);
        }
        Ok(())
    }
}

// state checkers
fn is_ok_to_deliver(params: &ReliableBroadcastParams, state: &DeliveryState) -> bool {
    match state.get(&SampleType::DeliverySubscription) {
        Some(sample) => match params.delivery_sample_size.checked_sub(sample.len()) {
            Some(consumed) => consumed >= params.delivery_threshold,
            None => false,
        },
        _ => false,
    }
}

fn is_e_ready(params: &ReliableBroadcastParams, state: &DeliveryState) -> bool {
    match state.get(&SampleType::EchoSubscription) {
        Some(sample) => match params.echo_sample_size.checked_sub(sample.len()) {
            Some(consumed) => consumed >= params.echo_threshold,
            None => false,
        },
        _ => false,
    }
}

fn is_r_ready(params: &ReliableBroadcastParams, state: &DeliveryState) -> bool {
    match state.get(&SampleType::ReadySubscription) {
        Some(sample) => match params.ready_sample_size.checked_sub(sample.len()) {
            Some(consumed) => consumed >= params.ready_threshold,
            None => false,
        },
        _ => false,
    }
}
