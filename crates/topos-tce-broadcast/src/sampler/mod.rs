use std::collections::HashSet;
use topos_p2p::PeerId;

/// Categorize what we expect from which peer for the broadcast
#[derive(PartialEq, Eq, Hash, Debug, Clone)]
pub enum SampleType {
    /// Listen Echo from this Sample
    EchoSubscription,
    /// Listen Ready from this Sample
    ReadySubscription,
    /// Send Echo to this Sample
    EchoSubscriber,
    /// Send Ready to this Sample
    ReadySubscriber,
}

/// Stateful network view with whom we broadcast the Certificate
/// The Echo and the Ready sets are initially equal to the whole network
#[derive(Debug, Clone, Eq, PartialEq, Default)]
pub struct SubscriptionsView {
    /// Set of Peer from which we listen Echo
    pub echo: HashSet<PeerId>,
    /// Set of Peer from which we listen Ready
    pub ready: HashSet<PeerId>,
    /// Size of the network
    pub network_size: usize,
}

impl SubscriptionsView {
    pub fn is_some(&self) -> bool {
        !self.is_none()
    }

    pub fn is_none(&self) -> bool {
        self.echo.is_empty() && self.ready.is_empty()
    }

    /// Current view of subscriptions of the node, which is initially the whole network
    pub fn get_subscriptions(&self) -> Vec<PeerId> {
        self.echo
            .iter()
            .chain(self.ready.iter())
            .cloned()
            .collect::<HashSet<_>>()
            .into_iter()
            .collect()
    }
}
