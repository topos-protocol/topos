use crate::Errors;
use crate::{sampler::SampleType, tce_store::TceStore, DoubleEchoCommand, SubscriptionsView};
use opentelemetry::trace::TraceContextExt;
use std::{
    collections::{HashMap, VecDeque},
    time,
};
use tce_transport::{ProtocolEvents, ReliableBroadcastParams};
use tokio::sync::{broadcast, mpsc, oneshot};
use topos_core::uci::{Certificate, CertificateId};
use topos_p2p::Client as NetworkClient;
use topos_p2p::PeerId;
use topos_tce_storage::{PendingCertificateId, StorageClient};
use tracing::{
    debug, error, info, info_span, instrument, trace, warn, warn_span, Instrument, Span,
};
use tracing_opentelemetry::OpenTelemetrySpanExt;

/// Processing data associated to a Certificate candidate for delivery
/// Sample repartition, one peer may belongs to multiple samples
#[derive(Clone)]
pub struct DeliveryState {
    pub subscriptions: SubscriptionsView,
    pub ready_sent: bool,
    pub delivered: bool,
    ctx: Span,
}

pub struct DoubleEcho {
    last_pending_certificate: PendingCertificateId,
    pub(crate) params: ReliableBroadcastParams,
    command_receiver: mpsc::Receiver<DoubleEchoCommand>,
    subscriptions_view_receiver: mpsc::Receiver<SubscriptionsView>,
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
    buffer: VecDeque<(bool, Certificate)>,
    pub(crate) shutdown: mpsc::Receiver<oneshot::Sender<()>>,

    local_peer_id: String,

    buffered_messages: HashMap<CertificateId, Vec<DoubleEchoCommand>>,
    max_buffer_size: usize,
}

impl DoubleEcho {
    pub const MAX_BUFFER_SIZE: usize = 2048;

    #[allow(clippy::too_many_arguments)]
    pub fn new(
        params: ReliableBroadcastParams,
        command_receiver: mpsc::Receiver<DoubleEchoCommand>,
        subscriptions_view_receiver: mpsc::Receiver<SubscriptionsView>,
        event_sender: broadcast::Sender<ProtocolEvents>,
        store: Box<dyn TceStore + Send>,
        storage: StorageClient,
        network_client: NetworkClient,
        shutdown: mpsc::Receiver<oneshot::Sender<()>>,
        local_peer_id: String,
        last_pending_certificate: PendingCertificateId,
        max_buffer_size: usize,
    ) -> Self {
        Self {
            last_pending_certificate,
            params,
            command_receiver,
            subscriptions_view_receiver,
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
            buffer: VecDeque::new(),
            shutdown,
            local_peer_id,
            buffered_messages: Default::default(),
            max_buffer_size,
        }
    }

    pub(crate) async fn run(mut self) {
        info!("DoubleEcho started");

        let shutdowned: Option<oneshot::Sender<()>> = loop {
            tokio::select! {
                shutdown = self.shutdown.recv() => {
                        warn!("Double echo shutdown signal received {:?}", shutdown);
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

                        DoubleEchoCommand::Broadcast { need_gossip, cert, ctx } => {
                            if self.storage.pending_certificate_exists(cert.id).await.is_err() &&
                                self.storage.get_certificate(cert.id).await.is_err()
                            {
                                let span = warn_span!("Broadcast", peer_id = self.local_peer_id, certificate_id = cert.id.to_string());
                                span.in_scope(|| {
                                    warn!("Broadcast registered for {}", cert.id);
                                    self.span_tracker.insert(cert.id, span.clone());
                                });
                                span.add_link(ctx.context().span().span_context().clone());
                                let maybe_pending = self
                                    .storage
                                    .add_pending_certificate(cert.clone())
                                    .instrument(span.clone())
                                    .await;

                                span.in_scope(||{
                                    info!("Certificate {} added to pending storage", cert.id);
                                    debug!("DoubleEchoCommand::Broadcast certificate_id: {}", cert.id);
                                    if self.buffer.len() < self.max_buffer_size {
                                        self.buffer.push_back((need_gossip, cert));
                                        if let Ok(pending) = maybe_pending {
                                            self.last_pending_certificate = pending;
                                        }
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

                else => {
                    warn!("Break the tokio loop for the double echo");
                    break None;
                }
            };

            #[cfg(not(feature = "direct"))]
            let has_subscriptions = self.subscriptions.is_some();

            #[cfg(feature = "direct")]
            let has_subscriptions = true;

            // Broadcast next certificate
            if has_subscriptions {
                // TODO: Remove the unused_variables attribute when the feature direct is removed
                #[allow(unused_variables)]
                if let Some((need_gossip, cert)) = self.buffer.pop_front() {
                    if let Some(ctx) = self.span_tracker.get(&cert.id) {
                        let span = info_span!(
                            parent: ctx,
                            "DoubleEcho start dispatching",
                            certificate_id = cert.id.to_string(),
                            peer_id = self.local_peer_id,
                            "otel.kind" = "producer"
                        );
                        let _span = span.entered();

                        let cert_id = cert.id;
                        #[cfg(feature = "direct")]
                        {
                            _ = self
                                .event_sender
                                .send(ProtocolEvents::CertificateDelivered { certificate: cert });
                        }
                        #[cfg(not(feature = "direct"))]
                        self.handle_broadcast(cert, need_gossip);

                        if let Some(messages) = self.buffered_messages.remove(&cert_id) {
                            for message in messages {
                                match message {
                                    DoubleEchoCommand::Echo {
                                        from_peer,
                                        certificate_id,
                                        ..
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

                                        let _enter = span.enter();
                                        self.handle_echo(from_peer, &certificate_id);
                                        self.state_change_follow_up();
                                    }
                                    DoubleEchoCommand::Ready {
                                        from_peer,
                                        certificate_id,
                                        ..
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

                                        let _enter = span.enter();
                                        self.handle_ready(from_peer, &certificate_id);

                                        self.state_change_follow_up();
                                    }
                                    _ => {}
                                }
                            }
                        }
                    } else {
                        warn!(
                            "No span found for certificate id: {} {:?}",
                            cert.id, cert.id
                        );
                    }
                    if let Ok(Some((pending, certificate))) = self
                        .storage
                        .next_pending_certificate(Some(self.last_pending_certificate as usize))
                        .await
                    {
                        self.last_pending_certificate = pending;
                        self.buffer.push_back((true, certificate));
                    } else {
                        info!("No more certificate to broadcast");
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
        }
    }

    #[cfg_attr(feature = "direct", allow(dead_code))]
    pub(crate) fn handle_broadcast(&mut self, cert: Certificate, origin: bool) {
        info!("🙌 Starting broadcasting the Certificate {}", &cert.id);

        self.dispatch(cert, origin);
    }

    pub(crate) fn handle_deliver(&mut self, cert: Certificate) {
        self.dispatch(cert, false)
    }

    /// Called to process potentially new certificate:
    /// - either submitted from API ( [tce_transport::TceCommands::Broadcast] command)
    /// - or received through the gossip (first step of protocol exchange)
    #[instrument(skip_all)]
    pub(crate) fn dispatch(&mut self, cert: Certificate, origin: bool) {
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

        let span = self
            .span_tracker
            .get(&cert.id)
            .cloned()
            .unwrap_or_else(Span::current);

        if origin {
            warn!("📣 Gossiping the Certificate {}", &cert.id);
            let _ = self.event_sender.send(ProtocolEvents::Gossip {
                cert: cert.clone(),
                ctx: span,
            });
        }

        // Trigger event of new certificate candidate for delivery
        self.start_broadcast(cert);
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

        let ctx = self
            .span_tracker
            .get(&cert.id)
            .cloned()
            .unwrap_or_else(Span::current);

        let _ = self.event_sender.send(ProtocolEvents::Echo {
            certificate_id: cert.id,
            ctx,
        });
    }

    /// Build initial delivery state
    fn delivery_state_for_new_cert(
        &mut self,
        certificate_id: &CertificateId,
    ) -> Option<DeliveryState> {
        let subscriptions = self.subscriptions.clone();

        // Check whether inbound sets are empty
        if subscriptions.echo.is_empty() || subscriptions.ready.is_empty() {
            error!(
                "One Subscription sample is empty: Echo({}), Ready({})",
                subscriptions.echo.is_empty(),
                subscriptions.ready.is_empty(),
            );
            None
        } else {
            let ctx = self
                .span_tracker
                .get(certificate_id)
                .cloned()
                .unwrap_or_else(Span::current);

            Some(DeliveryState {
                subscriptions,
                ready_sent: false,
                delivered: false,
                ctx,
            })
        }
    }

    pub(crate) fn state_change_follow_up(&mut self) {
        debug!("StateChangeFollowUp called");
        let mut state_modified = false;
        let mut gen_evts = Vec::<ProtocolEvents>::new();

        // For all current Cert on processing
        for (certificate_id, (certificate, state_to_delivery)) in &mut self.cert_candidate {
            // Check whether we should send Ready
            if !state_to_delivery.ready_sent
                && is_r_ready(
                    &self.params,
                    self.subscriptions.network_size,
                    state_to_delivery,
                )
            {
                // Fanout the Ready messages to my subscribers
                gen_evts.push(ProtocolEvents::Ready {
                    certificate_id: certificate.id,
                    ctx: state_to_delivery.ctx.clone(),
                });

                state_to_delivery.ready_sent = true;
                state_modified = true;
            }

            // Check whether we should deliver
            if !state_to_delivery.delivered
                && is_ok_to_deliver(
                    &self.params,
                    self.subscriptions.network_size,
                    state_to_delivery,
                )
            {
                self.pending_delivery.insert(
                    *certificate_id,
                    (certificate.clone(), state_to_delivery.ctx.clone()),
                );
                state_to_delivery.delivered = true;
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
                .checked_sub(state_to_delivery.subscriptions.ready.len())
                .map(|consumed| self.params.delivery_threshold.saturating_sub(consumed))
                .unwrap_or(0);

            debug!("Waiting for {echo_missing} Echo from the E-Sample");
            trace!("Echo Sample: {:?}", state_to_delivery.subscriptions.echo);

            debug!(
                "Waiting for {ready_missing}-R and {delivery_missing}-D Ready from the R-Sample"
            );
            trace!("Ready Sample: {:?}", state_to_delivery.subscriptions.ready);
        }

        if state_modified {
            // Keep the candidates only if not delivered, or not (E|R)-Ready yet
            self.cert_candidate
                .retain(|_, (_, state)| !state.delivered || !state.ready_sent);

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

/// Predicate on whether we reached the threshold to deliver the Certificate
fn is_ok_to_deliver(
    params: &ReliableBroadcastParams,
    network_size: usize,
    state: &DeliveryState,
) -> bool {
    // If reached the delivery threshold, I can deliver
    match network_size.checked_sub(state.subscriptions.ready.len()) {
        Some(consumed) => consumed >= params.delivery_threshold,
        None => false,
    }
}

/// Predicate on whether we reached the threshold to send our Ready for this Certificate
fn is_r_ready(
    params: &ReliableBroadcastParams,
    network_size: usize,
    state: &DeliveryState,
) -> bool {
    // Compute the threshold
    let reached_echo_threshold = match network_size.checked_sub(state.subscriptions.echo.len()) {
        Some(consumed) => consumed >= params.echo_threshold,
        None => false,
    };

    let reached_ready_threshold = match network_size.checked_sub(state.subscriptions.ready.len()) {
        Some(consumed) => consumed >= params.ready_threshold,
        None => false,
    };

    // If reached any of the Echo or Ready thresholds, I send the Ready
    reached_echo_threshold || reached_ready_threshold
}
