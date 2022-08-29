use crate::{
    sampler::{SampleType, SampleView},
    trb_store::TrbStore,
    DoubleEchoCommand, Peer,
};
use log::debug;
use std::{
    collections::{HashMap, HashSet},
    time,
};
use tce_transport::{ReliableBroadcastParams, TrbpEvents};
use tokio::sync::{broadcast, mpsc};
use topos_core::uci::{Certificate, CertificateId, DigestCompressed};

/// Processing data associated to a Certificate candidate for delivery
/// Sample repartition, one peer may belongs to multiple samples
type DeliveryState = HashMap<SampleType, HashSet<Peer>>;

pub struct DoubleEcho {
    my_peer_id: String,
    params: ReliableBroadcastParams,
    command_receiver: mpsc::Receiver<DoubleEchoCommand>,
    sample_view_receiver: mpsc::Receiver<SampleView>,
    event_sender: broadcast::Sender<TrbpEvents>,

    store: Box<dyn TrbStore + Send>,

    cert_candidate: HashMap<Certificate, DeliveryState>,
    pending_delivery: HashSet<Certificate>,
    all_known_certs: Vec<Certificate>,
    delivery_time: HashMap<CertificateId, (time::SystemTime, time::Duration)>,
    current_sample_view: Option<SampleView>,
}

impl DoubleEcho {
    pub fn new(
        my_peer_id: String,
        params: ReliableBroadcastParams,
        command_receiver: mpsc::Receiver<DoubleEchoCommand>,
        sample_view_receiver: mpsc::Receiver<SampleView>,
        event_sender: broadcast::Sender<TrbpEvents>,
        store: Box<dyn TrbStore + Send>,
    ) -> Self {
        Self {
            my_peer_id,
            params,
            command_receiver,
            sample_view_receiver,
            event_sender,
            store,
            cert_candidate: Default::default(),
            pending_delivery: Default::default(),
            all_known_certs: Default::default(),
            delivery_time: Default::default(),
            current_sample_view: Default::default(),
        }
    }

    pub(crate) async fn run(mut self) {
        loop {
            tokio::select! {
                Some(command) = self.command_receiver.recv() => {
                    if let DoubleEchoCommand::DeliveredCerts { subnet_id, limit, sender } = command {
                        let value = self
                            .store
                            .recent_certificates_for_subnet(&subnet_id, limit)
                            .iter()
                            .filter_map(|cert_id| self.store.cert_by_id(cert_id).ok())
                            .collect();

                        // TODO: Catch send failure
                        let _ = sender.send(Ok(value));
                    } else if self.current_sample_view.is_some() {
                        match command {
                            DoubleEchoCommand::Echo { from_peer, cert } => self.handle_echo(from_peer, cert),
                            DoubleEchoCommand::Ready { from_peer, cert } => self.handle_ready(from_peer, cert),
                            DoubleEchoCommand::Broadcast { cert } => self.handle_broadcast(cert),
                            DoubleEchoCommand::Deliver { cert, digest } => self.handle_deliver(cert, digest),
                            _ => {}
                        }

                        self.state_change_follow_up();
                    }

                }

                Some(new_sample_view) = self.sample_view_receiver.recv() => {
                    debug!("New sample receive on DoubleEcho {new_sample_view:?}");

                    self.current_sample_view = Some(new_sample_view);
                }
            }
        }
    }
}

impl DoubleEcho {
    fn sample_consume_peer(from_peer: &str, state: &mut DeliveryState, sample_type: SampleType) {
        if let Some(peers) = state.get_mut(&sample_type) {
            peers.remove(from_peer);
        }
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
        if let Some(computed_digest) = self.store.flush_digest_view(&cert.initial_subnet_id) {
            self.dispatch(cert, computed_digest);
        }
    }

    fn handle_deliver(&mut self, cert: Certificate, digest: DigestCompressed) {
        self.dispatch(cert, digest)
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

        let _ = self.event_sender.send(TrbpEvents::Gossip {
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
                let _ = self.event_sender.send(TrbpEvents::Die);
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
            log::warn!("[{:?}] EchoSubscriber peers set is empty", self.my_peer_id);
            return;
        }
        let _ = self.event_sender.send(TrbpEvents::Echo {
            peers: echo_peers,
            cert,
        });
    }

    /// build initial delivery state
    fn delivery_state_for_new_cert(&mut self) -> Option<DeliveryState> {
        let ds = self.current_sample_view.clone().unwrap();

        // check inbound sets are not empty
        if ds
            .get(&SampleType::EchoSubscription)
            .unwrap_or(&HashSet::<Peer>::new())
            .is_empty()
            || ds
                .get(&SampleType::ReadySubscription)
                .unwrap_or(&HashSet::<Peer>::new())
                .is_empty()
            || ds
                .get(&SampleType::DeliverySubscription)
                .unwrap_or(&HashSet::<Peer>::new())
                .is_empty()
        {
            None
        } else {
            Some(ds)
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
                    self.pending_delivery.remove(cert);
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
            let _ = self.event_sender.send(evt);
        }
    }

    /// Here comes test that can be done before delivery process
    /// in order to avoid going to delivery process for a Cert
    /// that is already known as incorrect
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
