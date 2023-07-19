use crate::constant;
use crate::{DoubleEchoCommand, SubscriptionsView};
use std::collections::HashSet;
use std::collections::{HashMap, VecDeque};
use tce_transport::{ProtocolEvents, ReliableBroadcastParams};
use tokio::sync::{mpsc, oneshot};
use topos_core::uci::{Certificate, CertificateId};
use topos_metrics::{
    CERTIFICATE_RECEIVED_FROM_API_TOTAL, CERTIFICATE_RECEIVED_FROM_GOSSIP_TOTAL,
    CERTIFICATE_RECEIVED_TOTAL, DOUBLE_ECHO_BROADCAST_CREATED_TOTAL,
    DOUBLE_ECHO_BUFFERED_MESSAGE_COUNT, DOUBLE_ECHO_BUFFER_CAPACITY_TOTAL,
    DOUBLE_ECHO_CURRENT_BUFFER_SIZE,
};
use topos_p2p::PeerId;
use tracing::{debug, error, info, warn, warn_span, Span};

use self::broadcast_state::BroadcastState;

mod broadcast_state;

/// Processing data associated to a Certificate candidate for delivery
/// Sample repartition, one peer may belongs to multiple samples
#[derive(Clone)]
pub struct DeliveryState {
    pub subscriptions: SubscriptionsView,
    pub ready_sent: bool,
    pub delivered: bool,
}

pub struct DoubleEcho {
    /// Channel to receive commands
    command_receiver: mpsc::Receiver<DoubleEchoCommand>,
    /// Channel to receive subscriptions updates
    subscriptions_view_receiver: mpsc::Receiver<SubscriptionsView>,
    /// Channel to send events
    event_sender: mpsc::Sender<ProtocolEvents>,

    /// Channel to receive shutdown signal
    pub(crate) shutdown: mpsc::Receiver<oneshot::Sender<()>>,

    /// pending certificates state
    pending_certificate_count: u64,
    /// buffer of certificates to process
    buffer: VecDeque<(bool, Certificate)>,

    /// known certificate ids to avoid processing twice the same certificate
    known_certificates: HashSet<CertificateId>,

    /// delivered certificate ids to avoid processing twice the same certificate
    delivered_certificates: HashSet<CertificateId>,

    pub(crate) params: ReliableBroadcastParams,

    /// Current certificates being processed
    cert_candidate: HashMap<CertificateId, BroadcastState>,

    /// Span tracker for each certificate
    span_tracker: HashMap<CertificateId, Span>,

    pub(crate) subscriptions: SubscriptionsView, // My subscriptions for echo, ready and delivery feedback

    local_peer_id: String,

    /// Buffer of messages to be processed once the certificate payload is received
    buffered_messages: HashMap<CertificateId, Vec<DoubleEchoCommand>>,
}

impl DoubleEcho {
    pub const MAX_BUFFER_SIZE: usize = 2048;

    #[allow(clippy::too_many_arguments)]
    pub fn new(
        params: ReliableBroadcastParams,
        command_receiver: mpsc::Receiver<DoubleEchoCommand>,
        subscriptions_view_receiver: mpsc::Receiver<SubscriptionsView>,
        event_sender: mpsc::Sender<ProtocolEvents>,
        shutdown: mpsc::Receiver<oneshot::Sender<()>>,
        local_peer_id: String,
        pending_certificate_count: u64,
    ) -> Self {
        Self {
            pending_certificate_count,
            params,
            command_receiver,
            subscriptions_view_receiver,
            event_sender,
            cert_candidate: Default::default(),
            span_tracker: Default::default(),
            subscriptions: SubscriptionsView::default(),
            buffer: VecDeque::new(),
            shutdown,
            local_peer_id,
            buffered_messages: Default::default(),
            delivered_certificates: Default::default(),
            known_certificates: Default::default(),
        }
    }

    /// DoubleEcho main loop
    ///   - Listen for shutdown signal
    ///   - Read new messages from command_receiver
    ///      - If a new certificate is received, add it to the buffer
    ///      - If a new subscription view is received, update the subscriptions
    ///      - If a new Echo/Ready is received, update the state of the certificate or buffer
    ///      the message
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

                        DoubleEchoCommand::Broadcast { need_gossip, cert } => self.handle_broadcast(cert,need_gossip),

                        command if self.subscriptions.is_some() => {
                            match command {
                                DoubleEchoCommand::Echo { from_peer, certificate_id } => self.handle_echo(from_peer, certificate_id),
                                DoubleEchoCommand::Ready { from_peer, certificate_id } => self.handle_ready(from_peer, certificate_id),
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

            // Broadcast next certificate
            if self.subscriptions.is_some() {
                if let Some((need_gossip, cert)) = self.buffer.pop_front() {
                    DOUBLE_ECHO_CURRENT_BUFFER_SIZE.dec();
                    let certificate_id = cert.id;

                    self.broadcast(cert, need_gossip);

                    if let Some(messages) = self.buffered_messages.remove(&certificate_id) {
                        for message in messages {
                            DOUBLE_ECHO_BUFFERED_MESSAGE_COUNT.dec();
                            match message {
                                DoubleEchoCommand::Echo {
                                    from_peer,
                                    certificate_id,
                                } => {
                                    self.consume_echo(from_peer, &certificate_id);
                                }
                                DoubleEchoCommand::Ready {
                                    from_peer,
                                    certificate_id,
                                } => {
                                    self.consume_ready(from_peer, &certificate_id);
                                }
                                _ => {}
                            }
                        }
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
    /// Called to process potentially new certificate:
    /// - either submitted from API ( [tce_transport::TceCommands::Broadcast] command)
    /// - or received through the gossip (first step of protocol exchange)
    pub(crate) fn broadcast(&mut self, cert: Certificate, origin: bool) {
        info!("🙌 Starting broadcasting the Certificate {}", &cert.id);

        if self.cert_pre_broadcast_check(&cert).is_err() {
            error!("Failure on the pre-check for the Certificate {}", &cert.id);
            self.event_sender
                .try_send(ProtocolEvents::BroadcastFailed {
                    certificate_id: cert.id,
                })
                .unwrap();
            return;
        }
        // Don't gossip one cert already gossiped
        if self.cert_candidate.contains_key(&cert.id) {
            self.event_sender
                .try_send(ProtocolEvents::BroadcastFailed {
                    certificate_id: cert.id,
                })
                .unwrap();
            return;
        }

        if self.delivered_certificates.get(&cert.id).is_some() {
            self.event_sender
                .try_send(ProtocolEvents::AlreadyDelivered {
                    certificate_id: cert.id,
                })
                .unwrap();

            return;
        }

        // Trigger event of new certificate candidate for delivery
        let certificate_id = cert.id;
        // To include tracing context in client requests from _this_ app,
        // use `context` to extract the current OpenTelemetry context.
        // Add new entry for the new Cert candidate
        match self.delivery_state_for_new_cert(cert, origin) {
            Some(delivery_state) => {
                self.cert_candidate.insert(certificate_id, delivery_state);
            }
            None => {
                error!("Ill-formed samples");
                _ = self.event_sender.try_send(ProtocolEvents::Die);
            }
        }
    }

    /// Build initial delivery state
    fn delivery_state_for_new_cert(
        &mut self,
        certificate: Certificate,
        origin: bool,
    ) -> Option<BroadcastState> {
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
            Some(BroadcastState::new(
                certificate,
                self.params.echo_threshold,
                self.params.ready_threshold,
                self.params.delivery_threshold,
                self.event_sender.clone(),
                subscriptions,
                origin,
            ))
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
}

impl DoubleEcho {
    pub(crate) fn handle_broadcast(&mut self, cert: Certificate, need_gossip: bool) {
        if !self.known_certificates.contains(&cert.id) {
            let span = warn_span!(
                "Broadcast",
                peer_id = self.local_peer_id,
                certificate_id = cert.id.to_string()
            );
            DOUBLE_ECHO_BROADCAST_CREATED_TOTAL.inc();
            span.in_scope(|| {
                warn!("Broadcast registered for {}", cert.id);
                self.span_tracker.insert(cert.id, span.clone());
                CERTIFICATE_RECEIVED_TOTAL.inc();

                if need_gossip {
                    CERTIFICATE_RECEIVED_FROM_API_TOTAL.inc();
                } else {
                    CERTIFICATE_RECEIVED_FROM_GOSSIP_TOTAL.inc();
                }
            });

            self.known_certificates.insert(cert.id);
            span.in_scope(|| {
                debug!("DoubleEchoCommand::Broadcast certificate_id: {}", cert.id);
                if self.buffer.len() < *constant::TOPOS_DOUBLE_ECHO_MAX_BUFFER_SIZE {
                    self.buffer.push_back((need_gossip, cert));
                    DOUBLE_ECHO_CURRENT_BUFFER_SIZE.inc();
                } else {
                    DOUBLE_ECHO_BUFFER_CAPACITY_TOTAL.inc();
                    // Adding one to the pending_certificate_count because we
                    // can't buffer it right now
                    _ = self.pending_certificate_count.checked_add(1);
                }
            });
        }
    }

    pub(crate) fn handle_echo(&mut self, from_peer: PeerId, certificate_id: CertificateId) {
        let cert_delivered = self.delivered_certificates.get(&certificate_id).is_some();
        if !cert_delivered {
            if self.known_certificates.get(&certificate_id).is_some() {
                debug!(
                    "Handling DoubleEchoCommand::Echo from_peer: {} cert_id: {}",
                    &from_peer, certificate_id
                );
                if let Some(status) = self.consume_echo(from_peer, &certificate_id) {
                    if status.is_delivered() {
                        self.delivered_certificates.insert(certificate_id);
                        self.span_tracker.remove(&certificate_id);
                        self.cert_candidate.remove(&certificate_id);
                    }
                }

                // need to deliver the certificate
            } else if self.delivered_certificates.get(&certificate_id).is_none() {
                // need to buffer the Echo
                self.buffered_messages
                    .entry(certificate_id)
                    .or_default()
                    .push(DoubleEchoCommand::Echo {
                        from_peer,
                        certificate_id,
                    });
                DOUBLE_ECHO_BUFFERED_MESSAGE_COUNT.inc();
            }
        }
    }

    pub(crate) fn handle_ready(&mut self, from_peer: PeerId, certificate_id: CertificateId) {
        let cert_delivered = self.delivered_certificates.get(&certificate_id).is_some();
        if !cert_delivered {
            if self.known_certificates.get(&certificate_id).is_some() {
                debug!(
                    "Handling DoubleEchoCommand::Ready from_peer: {} cert_id: {}",
                    &from_peer, &certificate_id
                );

                if let Some(status) = self.consume_ready(from_peer, &certificate_id) {
                    if status.is_delivered() {
                        self.delivered_certificates.insert(certificate_id);
                        self.span_tracker.remove(&certificate_id);
                        self.cert_candidate.remove(&certificate_id);
                    }
                }

                // need to deliver the certificate
            } else if self.delivered_certificates.get(&certificate_id).is_none() {
                // need to buffer the Ready
                self.buffered_messages
                    .entry(certificate_id)
                    .or_default()
                    .push(DoubleEchoCommand::Ready {
                        from_peer,
                        certificate_id,
                    });
                DOUBLE_ECHO_BUFFERED_MESSAGE_COUNT.inc();
            }
        }
    }

    pub(crate) fn consume_echo(
        &mut self,
        from_peer: PeerId,
        certificate_id: &CertificateId,
    ) -> Option<broadcast_state::Status> {
        self.cert_candidate
            .get_mut(certificate_id)
            .and_then(|state| state.apply_echo(from_peer))
    }

    pub(crate) fn consume_ready(
        &mut self,
        from_peer: PeerId,
        certificate_id: &CertificateId,
    ) -> Option<broadcast_state::Status> {
        self.cert_candidate
            .get_mut(certificate_id)
            .and_then(|state| state.apply_ready(from_peer))
    }
}
