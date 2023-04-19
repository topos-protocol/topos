use crate::sampler::SubscribersView;
use crate::Errors;
use crate::{
    sampler::SampleType, tce_store::TceStore, DoubleEchoCommand, SubscribersUpdate,
    SubscriptionsView,
};
use opentelemetry::trace::TraceContextExt;
use std::{
    collections::{HashMap, HashSet, VecDeque},
    time,
};
use tce_transport::{ProtocolEvents, ReliableBroadcastParams};
use tokio::sync::{broadcast, mpsc, oneshot};
use topos_core::uci::{Certificate, CertificateId};
use topos_p2p::Client as NetworkClient;
use topos_p2p::PeerId;
use topos_tce_storage::StorageClient;
use tracing::{
    debug, error, info, info_span, instrument, trace, warn, warn_span, Instrument, Span,
};
use tracing_opentelemetry::OpenTelemetrySpanExt;

/// Processing data associated to a Certificate candidate for delivery
/// Sample repartition, one peer may belongs to multiple samples
#[derive(Clone)]
pub struct DeliveryState {
    pub subscriptions: SubscriptionsView,
    ctx: Span,
}

pub struct DoubleEcho {
    pub(crate) params: ReliableBroadcastParams,
    command_receiver: mpsc::Receiver<DoubleEchoCommand>,
    subscriptions_view_receiver: mpsc::Receiver<SubscriptionsView>,
    subscribers_update_receiver: mpsc::Receiver<SubscribersUpdate>,
    event_sender: broadcast::Sender<ProtocolEvents>,
    store: Box<dyn TceStore + Send>,
    storage: StorageClient,
    #[allow(unused)]
    network_client: NetworkClient,

    cert_candidate: HashMap<CertificateId, (Certificate, DeliveryState)>,

    pending_delivery: HashMap<CertificateId, (Certificate, Span)>,
    span_tracker: HashMap<CertificateId, Span>,
    all_known_certs: Vec<Certificate>,
    delivery_time: HashMap<CertificateId, (time::SystemTime, time::Duration)>,
    pub(crate) subscriptions: SubscriptionsView, // My subscriptions for echo, ready and delivery feedback
    pub(crate) subscribers: SubscribersView,     // Echo and ready subscribers that are following me
    buffer: VecDeque<(Certificate, Span)>,
    pub(crate) shutdown: mpsc::Receiver<oneshot::Sender<()>>,

    local_peer_id: String,

    // root_span_tracker: HashMap<CertificateId, Span>,
    buffered_messages: HashMap<CertificateId, Vec<DoubleEchoCommand>>,
}

impl DoubleEcho {
    pub const MAX_BUFFER_SIZE: usize = 2048;

    #[allow(clippy::too_many_arguments)]
    pub fn new(
        params: ReliableBroadcastParams,
        command_receiver: mpsc::Receiver<DoubleEchoCommand>,
        subscriptions_view_receiver: mpsc::Receiver<SubscriptionsView>,
        subscribers_update_receiver: mpsc::Receiver<SubscribersUpdate>,
        event_sender: broadcast::Sender<ProtocolEvents>,
        store: Box<dyn TceStore + Send>,
        storage: StorageClient,
        network_client: NetworkClient,
        shutdown: mpsc::Receiver<oneshot::Sender<()>>,
        local_peer_id: String,
    ) -> Self {
        Self {
            params,
            command_receiver,
            subscriptions_view_receiver,
            subscribers_update_receiver,
            event_sender,
            store,
            storage,
            network_client,
            cert_candidate: Default::default(),
            pending_delivery: Default::default(),
            span_tracker: Default::default(),
            all_known_certs: Default::default(),
            delivery_time: Default::default(),
            subscriptions: SubscriptionsView::default(),
            subscribers: SubscribersView::default(),
            buffer: VecDeque::new(),
            shutdown,
            local_peer_id,
            // root_span_tracker: Default::default(),
            buffered_messages: Default::default(),
        }
    }

    pub(crate) async fn run(mut self) {
        info!("DoubleEcho started");
        let shutdowned: Option<oneshot::Sender<()>> = loop {
            tokio::select! {
                shutdown = self.shutdown.recv() => {
                        debug!("Double echo shutdown signal received {:?}", shutdown);
                        break shutdown;
                },
                Some(command) = self.command_receiver.recv() => {
                    match command {

                        DoubleEchoCommand::DeliveredCerts { subnet_id, limit, sender } => {

                            debug!("DoubleEchoCommand::DeliveredCerts, subnet_id: {:?}, limit: {}", &subnet_id, &limit);
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
                            debug!("DoubleEchoCommand::BroadcastMany cert ids: {:?}",
                                certificates.iter().map(|cert| &cert.id).collect::<Vec<&CertificateId>>());

                            self.buffer = certificates.into_iter().map(|c| (c, Span::current())).collect();
                        }

                        DoubleEchoCommand::Broadcast { cert, ctx} => {
                            if self.storage.pending_certificate_exists(cert.id).await.is_err() {
                                let span = warn_span!("Broadcast", peer_id = self.local_peer_id, certificate_id = cert.id.to_string());
                                span.add_link(ctx.context().span().span_context().clone());
                                _ = self
                                    .storage
                                    .add_pending_certificate(cert.clone())
                                    .await
                                    .instrument(span.clone());

                                span.in_scope(||{
                                    info!("Certificate {} added to pending storage", cert.id);
                                    debug!("DoubleEchoCommand::Broadcast certificate_id: {}", cert.id);
                                    if self.buffer.len() < Self::MAX_BUFFER_SIZE {
                                        self.span_tracker.insert(cert.id, span.clone());
                                        self.buffer.push_back((cert, Span::current()));
                                    } else {
                                        error!("Double echo buffer is full for certificate {}", cert.id);
                                        self.event_sender.send(ProtocolEvents::UnableToBufferCertificate {
                                            certificate_id: cert.id,
                                        }).unwrap();
                                    }
                                });
                            }
                        }

                        DoubleEchoCommand::IsCertificateDelivered { certificate_id, sender } => {
                            let _ = sender.send(self.store.cert_by_id(&certificate_id).is_ok());
                        }

                        DoubleEchoCommand::GetSpanOfCert { certificate_id, sender } => {
                            if let Some(ctx) = self.span_tracker.get(&certificate_id) {
                                _ = sender.send(Ok(ctx.clone()));
                            } else {
                                _ = sender.send(Err(Errors::CertificateNotFound));
                            }
                        }

                        command if self.subscriptions.is_some() => {
                            match command {
                                DoubleEchoCommand::Echo { from_peer, certificate_id, ctx } => {
                                    async {
                                        let cert_delivered = self.store.cert_by_id(&certificate_id).is_ok();
                                        if !cert_delivered {
                                            if self.storage
                                                .pending_certificate_exists(certificate_id)
                                                    .await
                                                    .is_ok()
                                            {
                                                let span = if let Some(root) = self.span_tracker.get(&certificate_id) {
                                                    info!("DEBUG::Receive ECHO with root");
                                                    info_span!(parent: root, "RECV Inbound Echo", peer = self.local_peer_id, certificate_id = certificate_id.to_string())
                                                } else {
                                                    info!("DEBUG::Receive ECHO without root");
                                                    info_span!("RECV Inbound Echo", peer = self.local_peer_id, certificate_id = certificate_id.to_string())
                                                };
                                                span.follows_from(ctx);

                                                let _enter = span.enter();
                                                debug!("Handling DoubleEchoCommand::Echo from_peer: {} cert_id: {}", &from_peer, certificate_id);
                                                self.handle_echo(from_peer, &certificate_id);

                                                self.state_change_follow_up();
                                                drop(_enter);
                                                // need to deliver the certificate
                                            } else if self.storage.get_certificate(certificate_id).await.is_err() {
                                                info!("DEBUG::Receive ECHO BUFFERING");
                                                // need to buffer the Echo
                                                self.buffered_messages
                                                    .entry(certificate_id)
                                                    .or_default()
                                                    .push(DoubleEchoCommand::Echo {
                                                        from_peer,
                                                        certificate_id,
                                                        ctx,
                                                    });
                                            }
                                        }
                                    }.await;
                                },
                                DoubleEchoCommand::Ready { from_peer, certificate_id, ctx } => {
                                    async {
                                        let cert_delivered = self.store.cert_by_id(&certificate_id).is_ok();
                                        if !cert_delivered {
                                            if self.storage
                                                .pending_certificate_exists(certificate_id)
                                                    .await
                                                    .is_ok()
                                            {
                                                let span =if let Some(root) = self.span_tracker.get(&certificate_id) {
                                                    info_span!(parent: root, "RECV Inbound Ready", peer = self.local_peer_id, certificate_id = certificate_id.to_string())
                                                } else {
                                                    info_span!("RECV Inbound Ready", peer = self.local_peer_id, certificate_id = certificate_id.to_string())
                                                };
                                                span.follows_from(ctx);

                                                let _enter = span.enter();
                                                debug!("Handling DoubleEchoCommand::Ready from_peer: {} cert_id: {}", &from_peer, &certificate_id);

                                                self.handle_ready(from_peer, &certificate_id);

                                                self.state_change_follow_up();
                                                drop(_enter);
                                                // need to deliver the certificate
                                            } else if self.storage.get_certificate(certificate_id).await.is_err() {
                                                // need to buffer the Ready
                                                self.buffered_messages
                                                    .entry(certificate_id)
                                                    .or_default()
                                                    .push(DoubleEchoCommand::Ready {
                                                        from_peer,
                                                        certificate_id,
                                                        ctx,
                                                    });
                                            }
                                        }
                                    }.await;
                                },
                                DoubleEchoCommand::Deliver { certificate_id, ctx, .. } => {
                                    let span = info_span!(parent: &ctx, "Handling Deliver", peer = self.local_peer_id, certificate_id = certificate_id.to_string());

                                    async {
                                    info!("Handling DoubleEchoCommand::Deliver cert_id: {}", certificate_id);
                                    if let Some((cert, _)) = self.cert_candidate.get(&certificate_id) {
                                        self.handle_deliver(cert.clone());
                                        self.state_change_follow_up();
                                    }
                                    }.instrument(span).await;
                                },


                                _ => {}
                            }

                        }
                        command => {
                            warn!("Received a command {command:?} while not having a complete sampling");
                        }
                    }
                }

                Some(new_subscriptions_view) = self.subscriptions_view_receiver.recv() => {
                    info!("Starting to use the new operational set of samples: {:?}", &new_subscriptions_view);
                    self.subscriptions = new_subscriptions_view;
                }

                Some(new_subscribers_update) = self.subscribers_update_receiver.recv() => {
                    info!( peer_id = self.local_peer_id,"Accepting new Subscribers: {:?}", &new_subscribers_update);
                    match new_subscribers_update {
                        SubscribersUpdate::NewEchoSubscriber(peer) => {
                            self.subscribers.echo.insert(peer);
                        }
                        SubscribersUpdate::NewReadySubscriber(peer) => {
                            self.subscribers.ready.insert(peer);
                        }
                        SubscribersUpdate::RemoveEchoSubscriber(peer) => {
                            self.subscribers.echo.remove(&peer);
                        }
                        SubscribersUpdate::RemoveEchoSubscribers(peers) => {
                            self.subscribers.echo = self.subscribers.echo.drain().filter(|v| !peers.contains(v)).collect();
                        }
                        SubscribersUpdate::RemoveReadySubscriber(peer) => {
                            self.subscribers.ready.remove(&peer);

                        }
                        SubscribersUpdate::RemoveReadySubscribers(peers) => {
                            self.subscribers.ready = self.subscribers.ready.drain().filter(|v| !peers.contains(v)).collect();
                        }
                    }
                    debug!("Subscribers are now: {:?}", self.subscribers);
                }
                else => {
                    debug!("Break the tokio loop for the double echo");
                    break None;
                }
            };

            #[cfg(not(feature = "direct"))]
            let has_subscriptions = self.subscriptions.is_some();

            #[cfg(feature = "direct")]
            let has_subscriptions = true;

            // Broadcast next certificate
            if has_subscriptions {
                if let Some((cert, ctx)) = self.buffer.pop_front() {
                    let span = info_span!(
                        parent: &ctx,
                        "DoubleEcho start dispatching",
                        certificate_id = cert.id.to_string(),
                        peer_id = self.local_peer_id,
                        "otel.kind" = "producer"
                    );
                    let _span = span.entered();

                    let cert_id = cert.id;
                    #[cfg(not(feature = "direct"))]
                    self.handle_broadcast(cert);

                    if let Some(messages) = self.buffered_messages.remove(&cert_id) {
                        for message in messages {
                            match message {
                                DoubleEchoCommand::Echo {
                                    from_peer,
                                    certificate_id,
                                    ctx,
                                } => {
                                    let span = if let Some(root) =
                                        self.span_tracker.get(&certificate_id)
                                    {
                                        info_span!(
                                            parent: root,
                                            "RECV Inbound Echo (Buffered)",
                                            peer = self.local_peer_id,
                                            certificate_id = certificate_id.to_string()
                                        )
                                    } else {
                                        info_span!(
                                            "RECV Inbound Echo (Buffered)",
                                            peer = self.local_peer_id,
                                            certificate_id = certificate_id.to_string()
                                        )
                                    };
                                    span.follows_from(ctx);

                                    let _enter = span.enter();
                                    self.handle_echo(from_peer, &certificate_id);
                                }
                                DoubleEchoCommand::Ready {
                                    from_peer,
                                    certificate_id,
                                    ctx,
                                } => {
                                    let span = if let Some(root) =
                                        self.span_tracker.get(&certificate_id)
                                    {
                                        info_span!(
                                            parent: root,
                                            "RECV Inbound Ready (Buffered)",
                                            peer = self.local_peer_id,
                                            certificate_id = certificate_id.to_string()
                                        )
                                    } else {
                                        info_span!(
                                            "RECV Inbound Ready (Buffered)",
                                            peer = self.local_peer_id,
                                            certificate_id = certificate_id.to_string()
                                        )
                                    };
                                    span.follows_from(ctx);

                                    let _enter = span.enter();
                                    self.handle_ready(from_peer, &certificate_id);
                                }
                                _ => {}
                            }
                        }
                    }
                    #[cfg(feature = "direct")]
                    {
                        _ = self
                            .event_sender
                            .send(ProtocolEvents::CertificateDelivered { certificate: cert });
                    }
                }
            }
        };

        if let Some(sender) = shutdowned {
            info!("Shutting down p2p double echo...");
            _ = sender.send(());
        } else {
            warn!("Shutting down p2p double echo due to error...");
        }
    }
}

impl DoubleEcho {
    fn sample_consume_peer(from_peer: &PeerId, state: &mut DeliveryState, sample_type: SampleType) {
        match sample_type {
            SampleType::EchoSubscription => state.subscriptions.echo.remove(from_peer),
            SampleType::ReadySubscription => state.subscriptions.ready.remove(from_peer),
            SampleType::DeliverySubscription => state.subscriptions.delivery.remove(from_peer),
            _ => false,
        };
    }

    pub(crate) fn handle_echo(&mut self, from_peer: PeerId, certificate_id: &CertificateId) {
        if let Some((_certificate, state)) = self.cert_candidate.get_mut(certificate_id) {
            Self::sample_consume_peer(&from_peer, state, SampleType::EchoSubscription);
        }
    }

    pub(crate) fn handle_ready(&mut self, from_peer: PeerId, certificate_id: &CertificateId) {
        if let Some((_certificate, state)) = self.cert_candidate.get_mut(certificate_id) {
            Self::sample_consume_peer(&from_peer, state, SampleType::ReadySubscription);
            Self::sample_consume_peer(&from_peer, state, SampleType::DeliverySubscription);
        }
    }

    #[cfg_attr(feature = "direct", allow(dead_code))]
    pub(crate) fn handle_broadcast(&mut self, cert: Certificate) {
        info!("🙌 Starting broadcasting the Certificate {}", &cert.id);

        self.dispatch(cert);
    }

    pub(crate) fn handle_deliver(&mut self, cert: Certificate) {
        self.dispatch(cert)
    }

    /// Called to process potentially new certificate:
    /// - either submitted from API ( [tce_transport::TceCommands::Broadcast] command)
    /// - or received through the gossip (first step of protocol exchange)
    #[instrument(skip_all)]
    pub(crate) fn dispatch(&mut self, cert: Certificate) {
        if self.cert_pre_broadcast_check(&cert).is_err() {
            error!("Failure on the pre-check for the Certificate {}", &cert.id);
            self.event_sender
                .send(ProtocolEvents::BroadcastFailed {
                    certificate_id: cert.id,
                })
                .unwrap();
            return;
        }
        // Don't gossip one cert already gossiped
        if self.cert_candidate.contains_key(&cert.id) {
            self.event_sender
                .send(ProtocolEvents::BroadcastFailed {
                    certificate_id: cert.id,
                })
                .unwrap();
            return;
        }

        if self.store.cert_by_id(&cert.id).is_ok() {
            self.event_sender
                .send(ProtocolEvents::AlreadyDelivered {
                    certificate_id: cert.id,
                })
                .unwrap();

            return;
        }

        // Gossip the certificate to all my peers
        let gossip_peers = self.gossip_peers();
        info!(
            "Gossiping the Certificate {} to the Gossip peers: {:?}",
            cert.id, &gossip_peers
        );

        // let root_ctx = self
        //     .root_span_tracker
        //     .get(&cert.id)
        //     .cloned()
        //     .expect("Unable to find root span for the certificate");

        let span = self
            .span_tracker
            .get(&cert.id)
            .cloned()
            .unwrap_or_else(Span::current);

        let _ = self.event_sender.send(ProtocolEvents::Gossip {
            peers: gossip_peers, // considered as the G-set for erdos-renyi
            cert: cert.clone(),
            ctx: span,
        });

        // Trigger event of new certificate candidate for delivery
        self.start_broadcast(cert);
    }

    /// Make gossip peer list from echo and ready
    /// subscribers that listen to me
    pub(crate) fn gossip_peers(&self) -> Vec<PeerId> {
        self.subscriptions
            .get_subscriptions()
            .into_iter()
            .chain(self.subscribers.get_subscribers().into_iter())
            .collect::<HashSet<_>>() //Filter duplicates
            .into_iter()
            .collect()
    }

    fn start_broadcast(&mut self, cert: Certificate) {
        // To include tracing context in client requests from _this_ app,
        // use `context` to extract the current OpenTelemetry context.
        // Add new entry for the new Cert candidate
        match self.delivery_state_for_new_cert(&cert.id) {
            Some(delivery_state) => {
                info!("DeliveryState is : {:?}", delivery_state.subscriptions);

                self.cert_candidate
                    .insert(cert.id, (cert.clone(), delivery_state));

                _ = self.event_sender.send(ProtocolEvents::Broadcast {
                    certificate_id: cert.id,
                });
            }
            None => {
                error!("Ill-formed samples");
                let _ = self.event_sender.send(ProtocolEvents::Die);
                return;
            }
        }
        self.all_known_certs.push(cert.clone());
        self.delivery_time
            .insert(cert.id, (time::SystemTime::now(), Default::default()));
        // Send Echo to the echo sample
        let echo_peers = self.subscribers.echo.iter().cloned().collect::<Vec<_>>();
        if echo_peers.is_empty() {
            warn!("The sample of Echo Subscribers is empty");
            return;
        }

        let ctx = self
            .span_tracker
            .get(&cert.id)
            .cloned()
            .unwrap_or_else(Span::current);

        let _ = self.event_sender.send(ProtocolEvents::Echo {
            peers: echo_peers,
            certificate_id: cert.id,
            ctx,
            // root_ctx,
        });
    }

    /// Build initial delivery state
    fn delivery_state_for_new_cert(
        &mut self,
        certificate_id: &CertificateId,
    ) -> Option<DeliveryState> {
        let subscriptions = self.subscriptions.clone();
        // check inbound sets are not empty

        if subscriptions.echo.is_empty()
            || subscriptions.ready.is_empty()
            || subscriptions.delivery.is_empty()
        {
            error!(
                "One Subscription sample is empty: Echo({}), Ready({}), Delivery({})",
                subscriptions.echo.is_empty(),
                subscriptions.ready.is_empty(),
                subscriptions.delivery.is_empty()
            );
            None
        } else {
            let ctx = self
                .span_tracker
                .get(certificate_id)
                .cloned()
                .unwrap_or_else(Span::current);

            Some(DeliveryState { subscriptions, ctx })
        }
    }

    pub(crate) fn state_change_follow_up(&mut self) {
        debug!("StateChangeFollowUp called");
        let mut state_modified = false;
        let mut gen_evts = Vec::<ProtocolEvents>::new();
        // For all current Cert on processing
        for (certificate_id, (certificate, state_to_delivery)) in &mut self.cert_candidate {
            if !state_to_delivery.subscriptions.ready.is_empty()
                && (is_e_ready(
                    &self.params,
                    self.subscriptions.network_size,
                    state_to_delivery,
                ) || is_r_ready(
                    &self.params,
                    self.subscriptions.network_size,
                    state_to_delivery,
                ))
            {
                // Fanout the Ready messages to my subscribers
                let readies = self.subscribers.ready.iter().cloned().collect::<Vec<_>>();
                if !readies.is_empty() {
                    gen_evts.push(ProtocolEvents::Ready {
                        peers: readies.clone(),
                        certificate_id: certificate.id,
                        ctx: state_to_delivery.ctx.clone(),
                    });
                }
                state_to_delivery.subscriptions.ready.clear();
                state_to_delivery.subscriptions.echo.clear();
                state_modified = true;
            }

            if !state_to_delivery.subscriptions.delivery.is_empty()
                && is_ok_to_deliver(
                    &self.params,
                    self.subscriptions.network_size,
                    state_to_delivery,
                )
            {
                state_to_delivery.subscriptions.delivery.clear();
                self.pending_delivery.insert(
                    *certificate_id,
                    (certificate.clone(), state_to_delivery.ctx.clone()),
                );
                state_modified = true;
            }

            let echo_missing = self
                .subscriptions
                .network_size
                .checked_sub(state_to_delivery.subscriptions.echo.len())
                .map(|consumed| self.params.echo_threshold.saturating_sub(consumed))
                .unwrap_or(0);
            let ready_missing = self
                .subscriptions
                .network_size
                .checked_sub(state_to_delivery.subscriptions.ready.len())
                .map(|consumed| self.params.ready_threshold.saturating_sub(consumed))
                .unwrap_or(0);
            let delivery_missing = self
                .subscriptions
                .network_size
                .checked_sub(state_to_delivery.subscriptions.delivery.len())
                .map(|consumed| self.params.delivery_threshold.saturating_sub(consumed))
                .unwrap_or(0);

            debug!("Waiting for {echo_missing} Echo from the E-Sample");
            trace!("Echo Sample: {:?}", state_to_delivery.subscriptions.echo);

            debug!("Waiting for {ready_missing} Ready from the R-Sample");
            trace!("Ready Sample: {:?}", state_to_delivery.subscriptions.ready);

            debug!("Waiting for {delivery_missing} Ready from the D-Sample");
            trace!(
                "Delivery Sample: {:?}",
                state_to_delivery.subscriptions.delivery
            );
        }

        if state_modified {
            // Keep the candidates only if not delivered, or not (E|R)-Ready yet
            self.cert_candidate.retain(|c, (_, state)| {
                !self.pending_delivery.contains_key(c) || !state.subscriptions.ready.is_empty()
            });

            let delivered_certificates = self
                .pending_delivery
                .iter()
                .filter(|(_, (c, _))| self.cert_post_delivery_check(c).is_ok())
                .map(|(c, s)| (*c, s.clone()))
                .collect::<HashMap<_, _>>();

            for (certificate_id, (certificate, ctx)) in delivered_certificates {
                let span = info_span!(parent: &ctx, "Delivered");

                span.in_scope(|| {
                    let mut d = time::Duration::from_millis(0);
                    if let Some((from, duration)) = self.delivery_time.get_mut(&certificate_id) {
                        *duration = from.elapsed().unwrap();
                        d = *duration;

                        info!("Certificate {} got delivered in {:?}", certificate_id, d);
                    }
                    self.pending_delivery.remove(&certificate_id);
                    self.cert_candidate.remove(&certificate_id);
                    self.span_tracker.remove(&certificate_id);

                    debug!("📝 Accepted[{}]\t Delivery time: {:?}", &certificate_id, d);

                    _ = self
                        .event_sender
                        .send(ProtocolEvents::CertificateDelivered {
                            certificate: certificate.clone(),
                        });

                    self.store
                        .add_cert_in_hist(&certificate.source_subnet_id, &certificate);
                });
            }
        }

        for evt in gen_evts {
            let _ = self.event_sender.send(evt);
        }
    }

    /// Checks done before starting to broadcast
    fn cert_pre_broadcast_check(&self, cert: &Certificate) -> Result<(), ()> {
        if cert.check_signature().is_err() {
            error!("Error on the signature");
        }

        if cert.check_proof().is_err() {
            error!("Error on the proof");
        }

        Ok(())
    }

    /// Here comes test that is necessarily done after delivery
    fn cert_post_delivery_check(&self, _cert: &Certificate) -> Result<(), ()> {
        Ok(())
    }
}

// state checkers
fn is_ok_to_deliver(
    params: &ReliableBroadcastParams,
    network_size: usize,
    state: &DeliveryState,
) -> bool {
    match network_size.checked_sub(state.subscriptions.delivery.len()) {
        Some(consumed) => consumed >= params.delivery_threshold,
        None => false,
    }
}

fn is_e_ready(
    params: &ReliableBroadcastParams,
    network_size: usize,
    state: &DeliveryState,
) -> bool {
    match network_size.checked_sub(state.subscriptions.echo.len()) {
        Some(consumed) => consumed >= params.echo_threshold,
        None => false,
    }
}

fn is_r_ready(
    params: &ReliableBroadcastParams,
    network_size: usize,
    state: &DeliveryState,
) -> bool {
    match network_size.checked_sub(state.subscriptions.ready.len()) {
        Some(consumed) => consumed >= params.ready_threshold,
        None => false,
    }
}
