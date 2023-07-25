use crate::task_manager_futures::task::TaskStatus;
use crate::{DoubleEchoCommand, SubscriptionsView};
use std::collections::HashSet;
use std::collections::{HashMap, VecDeque};
use tce_transport::{ProtocolEvents, ReliableBroadcastParams};
use tokio::sync::{broadcast, mpsc, oneshot};
use topos_core::uci::{Certificate, CertificateId};

use topos_p2p::PeerId;
use tracing::{error, info, warn, Span};

use self::broadcast_state::BroadcastState;

pub mod broadcast_state;

pub struct DoubleEcho {
    /// Channel to receive commands
    command_receiver: mpsc::Receiver<DoubleEchoCommand>,
    /// Channel to receive subscriptions updates
    subscriptions_view_receiver: broadcast::Receiver<SubscriptionsView>,
    /// Channel to send events
    event_sender: mpsc::Sender<ProtocolEvents>,

    /// Channel to receive shutdown signal
    pub(crate) shutdown: mpsc::Receiver<oneshot::Sender<()>>,

    /// pending certificates state
    pending_certificate_count: u64,

    /// buffer of certificates to process
    buffer: VecDeque<(bool, Certificate)>,

    /// delivered certificate ids to avoid processing twice the same certificate
    delivered_certificates: HashSet<CertificateId>,

    pub(crate) params: ReliableBroadcastParams,

    task_manager_message_sender: mpsc::Sender<DoubleEchoCommand>,

    task_completion_receiver: mpsc::Receiver<(CertificateId, TaskStatus)>,

    /// Current certificates being processed
    cert_candidate: HashMap<CertificateId, BroadcastState>,

    /// Span tracker for each certificate
    span_tracker: HashMap<CertificateId, Span>,

    pub(crate) subscriptions: SubscriptionsView, // My subscriptions for echo, ready and delivery feedback

    local_peer_id: String,
}

impl DoubleEcho {
    pub const MAX_BUFFER_SIZE: usize = 2048;

    #[allow(clippy::too_many_arguments)]
    pub fn new(
        params: ReliableBroadcastParams,
        task_manager_message_sender: mpsc::Sender<DoubleEchoCommand>,
        task_completion_receiver: mpsc::Receiver<(CertificateId, TaskStatus)>,
        command_receiver: mpsc::Receiver<DoubleEchoCommand>,
        subscriptions_view_receiver: broadcast::Receiver<SubscriptionsView>,
        event_sender: mpsc::Sender<ProtocolEvents>,
        shutdown: mpsc::Receiver<oneshot::Sender<()>>,
        local_peer_id: String,
        pending_certificate_count: u64,
    ) -> Self {
        Self {
            params,
            task_manager_message_sender,
            task_completion_receiver,
            pending_certificate_count,
            command_receiver,
            subscriptions_view_receiver,
            event_sender,
            cert_candidate: Default::default(),
            span_tracker: Default::default(),
            subscriptions: SubscriptionsView::default(),
            buffer: VecDeque::new(),
            shutdown,
            local_peer_id,
            delivered_certificates: Default::default(),
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

                        DoubleEchoCommand::Broadcast { need_gossip, ref cert } => self.task_manager_message_sender.send(command).await,

                        command if self.subscriptions.is_some() => Ok({
                            match command {
                                DoubleEchoCommand::Echo { from_peer, certificate_id } => self.handle_echo(from_peer, certificate_id).await,
                                DoubleEchoCommand::Ready { from_peer, certificate_id } => self.handle_ready(from_peer, certificate_id).await,
                                _ => {}
                            }

                        }),
                        command => {
                            warn!("Received a command {command:?} while not having a complete sampling");
                            Ok(())
                        }
                    }
                }

                Some((certificate_id, status)) = self.task_completion_receiver.recv() => {
                    if status == TaskStatus::Success {
                        self.delivered_certificates.insert(certificate_id);
                    }
                    Ok(())
                }

                Ok(new_subscriptions_view) = self.subscriptions_view_receiver.recv() => {
                    info!("Starting to use the new operational set of samples: {:?}", &new_subscriptions_view);
                    self.subscriptions = new_subscriptions_view;
                    Ok(())
                }

                else => {
                    warn!("Break the tokio loop for the double echo");
                    break None;
                }
            };
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
    pub(crate) async fn handle_echo(&mut self, from_peer: PeerId, certificate_id: CertificateId) {
        if self.delivered_certificates.get(&certificate_id).is_none() {
            let _ = self
                .task_manager_message_sender
                .send(DoubleEchoCommand::Echo {
                    from_peer,
                    certificate_id,
                })
                .await;
        }
    }

    pub(crate) async fn handle_ready(&mut self, from_peer: PeerId, certificate_id: CertificateId) {
        if self.delivered_certificates.get(&certificate_id).is_none() {
            println!("SEND READY");
            let _ = self
                .task_manager_message_sender
                .send(DoubleEchoCommand::Ready {
                    from_peer,
                    certificate_id,
                })
                .await;
        }
    }
}
