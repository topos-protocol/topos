use crate::double_echo::broadcast_state::BroadcastState;
use crate::double_echo::task::Task;
use crate::TaskStatus;
use crate::{DoubleEchoCommand, SubscriptionsView};
use futures::{stream::FuturesUnordered, StreamExt};
use std::future::IntoFuture;
use std::{
    collections::{HashMap, HashSet},
    future::Future,
    pin::Pin,
};
use task::TaskContext;
use tce_transport::{ProtocolEvents, ReliableBroadcastParams};
use tokio::spawn;
use tokio::sync::{mpsc, oneshot};
use topos_core::uci::{Certificate, CertificateId};
use topos_metrics::DOUBLE_ECHO_ACTIVE_TASKS_COUNT;
use topos_p2p::PeerId;
use tracing::{error, info, warn};

pub mod broadcast_state;
pub mod task;

pub struct DoubleEcho {
    /// If Echo | Ready messages arrive before the related certificate is broadcasted, they are buffered here
    pub buffered_messages: HashMap<CertificateId, Vec<DoubleEchoCommand>>,
    /// Channel to receive DoubleEchoCommands from the TCE process
    command_receiver: mpsc::Receiver<DoubleEchoCommand>,
    /// Delivered certificate ids to avoid processing twice the same certificate
    delivered_certificates: HashSet<CertificateId>,
    /// Channel to send back ProtocolEvents to the TCE process
    event_sender: mpsc::Sender<ProtocolEvents>,
    /// The threshold parameters for the double echo
    pub params: ReliableBroadcastParams,
    /// The running tasks, which end themselves when they are done
    #[allow(clippy::type_complexity)]
    pub running_tasks: FuturesUnordered<
        Pin<Box<dyn Future<Output = (CertificateId, TaskStatus)> + Send + 'static>>,
    >,
    /// Channel to receive shutdown signal
    pub(crate) shutdown: mpsc::Receiver<oneshot::Sender<()>>,
    /// The overview of the network, which holds echo and ready subscriptions and the network size
    pub subscriptions: SubscriptionsView,
    /// All open tasks, which currently handle processing a certificate each
    pub tasks: HashMap<CertificateId, TaskContext>,
}

impl DoubleEcho {
    pub const MAX_BUFFER_SIZE: usize = 2048;

    #[allow(clippy::too_many_arguments)]
    pub fn new(
        params: ReliableBroadcastParams,
        command_receiver: mpsc::Receiver<DoubleEchoCommand>,
        event_sender: mpsc::Sender<ProtocolEvents>,
        shutdown: mpsc::Receiver<oneshot::Sender<()>>,
    ) -> Self {
        Self {
            buffered_messages: Default::default(),
            command_receiver,
            delivered_certificates: Default::default(),
            event_sender,
            params,
            running_tasks: FuturesUnordered::new(),
            shutdown,
            subscriptions: SubscriptionsView::default(),
            tasks: HashMap::new(),
        }
    }

    /// DoubleEcho main loop
    ///   - Listen for shutdown signal
    ///   - Read new messages from command_receiver
    ///      - If a new certificate is received, add it to the buffer
    ///      - If a new subscription view is received, update the subscriptions
    ///      - If a new Echo/Ready is received, update the state of the certificate or buffer
    ///      the message
    pub async fn run(mut self, mut subscriptions_view_receiver: mpsc::Receiver<SubscriptionsView>) {
        let shutdowned: Option<oneshot::Sender<()>> = loop {
            tokio::select! {
                biased;

                Some(new_subscriptions_view) = subscriptions_view_receiver.recv() => {
                    self.subscriptions = new_subscriptions_view;
                }

                Some(command) = self.command_receiver.recv() => {
                    match command {

                        DoubleEchoCommand::Broadcast { need_gossip, cert } => self.broadcast(cert, need_gossip).await,

                        command if self.subscriptions.is_some() => {
                            match command {
                                DoubleEchoCommand::Echo { from_peer, certificate_id, .. } => self.handle_echo(from_peer, certificate_id).await,
                                DoubleEchoCommand::Ready { from_peer, certificate_id } => self.handle_ready(from_peer, certificate_id).await,
                                _ => {}
                            }

                        },
                        command => {
                            warn!("Received a command {command:?} while not having a complete sampling");
                        }
                    }
                }

                Some((certificate_id, status)) = self.running_tasks.next() => {
                    if status == TaskStatus::Success {
                        self.tasks.remove(&certificate_id);
                        DOUBLE_ECHO_ACTIVE_TASKS_COUNT.dec();}
                }

                shutdown = self.shutdown.recv() => {
                        warn!("Double echo shutdown signal received {:?}", shutdown);
                        break shutdown;
                },


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
    pub async fn broadcast(&mut self, cert: Certificate, origin: bool) {
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

        if self.delivered_certificates.get(&cert.id).is_some() {
            self.event_sender
                .try_send(ProtocolEvents::AlreadyDelivered {
                    certificate_id: cert.id,
                })
                .unwrap();

            return;
        }

        if self
            .delivery_state_for_new_cert(cert, origin)
            .await
            .is_none()
        {
            error!("Ill-formed samples");
            _ = self.event_sender.try_send(ProtocolEvents::Die);
        }
    }

    /// Build initial delivery state
    async fn delivery_state_for_new_cert(
        &mut self,
        certificate: Certificate,
        origin: bool,
    ) -> Option<bool> {
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
            match self.tasks.entry(certificate.id) {
                std::collections::hash_map::Entry::Vacant(entry) => {
                    let broadcast_state = BroadcastState::new(
                        certificate.clone(),
                        self.params.echo_threshold,
                        self.params.ready_threshold,
                        self.params.delivery_threshold,
                        self.event_sender.clone(),
                        self.subscriptions.clone(),
                        origin,
                    );

                    let (task, task_context) = Task::new(certificate.id, broadcast_state);

                    self.running_tasks.push(task.into_future());

                    if let Some(messages) = self.buffered_messages.remove(&certificate.id) {
                        let sink = task_context.sink.clone();
                        spawn(async move {
                            for msg in messages {
                                _ = sink.send(msg).await;
                            }
                        });
                    }

                    DOUBLE_ECHO_ACTIVE_TASKS_COUNT.inc();
                    entry.insert(task_context);
                }
                std::collections::hash_map::Entry::Occupied(_) => {}
            }
            Some(true)
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
    pub async fn handle_echo(&mut self, from_peer: PeerId, certificate_id: CertificateId) {
        if self.delivered_certificates.get(&certificate_id).is_none() {
            if let Some(task_context) = self.tasks.get(&certificate_id) {
                let _ = task_context
                    .sink
                    .send(DoubleEchoCommand::Echo {
                        from_peer,
                        certificate_id,
                    })
                    .await;
            } else {
                self.buffered_messages
                    .entry(certificate_id)
                    .or_default()
                    .push(DoubleEchoCommand::Echo {
                        from_peer,
                        certificate_id,
                    });
            };
        }
    }

    pub async fn handle_ready(&mut self, from_peer: PeerId, certificate_id: CertificateId) {
        if self.delivered_certificates.get(&certificate_id).is_none() {
            if let Some(task_context) = self.tasks.get(&certificate_id) {
                _ = task_context
                    .sink
                    .send(DoubleEchoCommand::Ready {
                        from_peer,
                        certificate_id,
                    })
                    .await;
            } else {
                self.buffered_messages
                    .entry(certificate_id)
                    .or_default()
                    .push(DoubleEchoCommand::Ready {
                        from_peer,
                        certificate_id,
                    });
            };
        }
    }
}
