use std::{collections::HashSet, time};

use tce_transport::ProtocolEvents;
use tokio::sync::mpsc;
use topos_core::uci::Certificate;
use topos_metrics::DOUBLE_ECHO_BROADCAST_FINISHED_TOTAL;
use topos_p2p::PeerId;
use topos_tce_storage::{
    types::{CertificateDelivered, ProofOfDelivery, Ready, SourceStreamPositionKey},
    Position,
};
use tracing::{debug, info, warn};

use crate::sampler::SubscriptionsView;

mod status;

pub use status::Status;

#[derive(Debug)]
pub struct BroadcastState {
    subscriptions_view: SubscriptionsView,
    status: Status,
    pub(crate) certificate: Certificate,
    echo_threshold: usize,
    ready_threshold: usize,
    delivery_threshold: usize,
    event_sender: mpsc::Sender<ProtocolEvents>,
    delivery_time: time::Instant,
    readies: HashSet<Ready>,
    pub(crate) expected_position: Option<Position>,
}

impl BroadcastState {
    pub fn new(
        certificate: Certificate,
        echo_threshold: usize,
        ready_threshold: usize,
        delivery_threshold: usize,
        event_sender: mpsc::Sender<ProtocolEvents>,
        subscriptions_view: SubscriptionsView,
        need_gossip: bool,
    ) -> Self {
        let mut state = Self {
            subscriptions_view,
            status: Status::Pending,
            certificate,
            echo_threshold,
            ready_threshold,
            delivery_threshold,
            event_sender,
            delivery_time: time::Instant::now(),
            readies: HashSet::new(),
            expected_position: None,
        };

        _ = state.event_sender.try_send(ProtocolEvents::Broadcast {
            certificate_id: state.certificate.id,
        });

        if need_gossip {
            warn!("📣 Gossiping the Certificate {}", &state.certificate.id);
            let _ = state.event_sender.try_send(ProtocolEvents::Gossip {
                cert: state.certificate.clone(),
            });
        }

        state.update_status();

        state
    }

    pub fn into_delivered(&self) -> CertificateDelivered {
        CertificateDelivered {
            certificate: self.certificate.clone(),
            proof_of_delivery: ProofOfDelivery {
                certificate_id: self.certificate.id,
                delivery_position: SourceStreamPositionKey(
                    self.certificate.source_subnet_id,
                    // FIXME: Should never fails but need to find how to remove the unwrap
                    self.expected_position
                        .expect("Expected position is not set, this is a bug"),
                ),
                readies: self
                    .readies
                    .iter()
                    .cloned()
                    .map(|r| (r, "signature".to_string()))
                    .collect(),
                threshold: self.delivery_threshold as u64,
            },
        }
    }

    pub fn apply_echo(&mut self, peer_id: PeerId) -> Option<Status> {
        if self.subscriptions_view.echo.remove(&peer_id) {
            self.update_status()
        } else {
            None
        }
    }

    pub fn apply_ready(&mut self, peer_id: PeerId) -> Option<Status> {
        if self.subscriptions_view.ready.remove(&peer_id) {
            self.readies.insert(peer_id.to_string());
            self.update_status()
        } else {
            None
        }
    }

    fn update_status(&mut self) -> Option<Status> {
        // Nothing happened yet, we're in the initial state and didn't Procced
        // any Echo or Ready messages
        // Sending our Echo message
        if let Status::Pending = self.status {
            _ = self.event_sender.try_send(ProtocolEvents::Echo {
                certificate_id: self.certificate.id,
            });

            self.status = Status::EchoSent;
            debug!(
                "📝 Certificate {} is now {}",
                &self.certificate.id, self.status
            );
            return Some(self.status);
        }

        // Upon reaching the Echo or Ready threshold, if the status is either
        // EchoSent or Delivered (without ReadySent), we send the Ready message
        // and update the status accordingly.
        // If the status was EchoSent, we update it to ReadySent
        // If the status was Delivered, we update it to DeliveredWithReadySent
        if !self.status.is_ready_sent() && self.reached_ready_threshold() {
            let event = ProtocolEvents::Ready {
                certificate_id: self.certificate.id,
            };
            if let Err(e) = self.event_sender.try_send(event) {
                warn!("Error sending Ready message: {}", e);
            }

            self.status = self.status.ready_sent();

            debug!(
                "📝 Certificate {} is now {}",
                &self.certificate.id, self.status
            );
            return Some(self.status);
        }

        // Upon reaching the Delivery threshold, if the status is not Delivered,
        // we update the status to Delivered and change the status
        if !self.status.is_delivered() && self.reached_delivery_threshold() {
            self.status = self.status.delivered();

            debug!(
                "📝 Certificate {} is now {}",
                &self.certificate.id, self.status
            );
            // Calculate delivery time
            let from = self.delivery_time;
            let duration = from.elapsed();
            let d = duration;

            info!(
                "Certificate {} got delivered in {:?}",
                self.certificate.id, d
            );

            debug!(
                "📝 Accepted[{}]\t Delivery time: {:?}",
                &self.certificate.id, d
            );

            DOUBLE_ECHO_BROADCAST_FINISHED_TOTAL.inc();

            return Some(self.status);
        }

        None
    }

    fn reached_ready_threshold(&self) -> bool {
        // Compute the threshold
        let reached_echo_threshold = match self
            .subscriptions_view
            .network_size
            .checked_sub(self.subscriptions_view.echo.len())
        {
            Some(consumed) => consumed >= self.echo_threshold,
            None => false,
        };

        let reached_ready_threshold = match self
            .subscriptions_view
            .network_size
            .checked_sub(self.subscriptions_view.ready.len())
        {
            Some(consumed) => consumed >= self.ready_threshold,
            None => false,
        };

        debug!(
            "📝 Certificate {} reached Echo threshold: {} and Ready threshold: {}",
            &self.certificate.id, reached_echo_threshold, reached_ready_threshold
        );
        // If reached any of the Echo or Ready thresholds, I send the Ready
        reached_echo_threshold || reached_ready_threshold
    }

    fn reached_delivery_threshold(&self) -> bool {
        // If reached the delivery threshold, I can deliver
        match self
            .subscriptions_view
            .network_size
            .checked_sub(self.subscriptions_view.ready.len())
        {
            Some(consumed) => consumed >= self.delivery_threshold,
            None => false,
        }
    }
}
