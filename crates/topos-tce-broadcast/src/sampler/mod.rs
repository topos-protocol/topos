use std::collections::HashSet;
use topos_p2p::PeerId;

// These are all the same in the new, deterministic view (Subscriptions == Subscribers)
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

#[derive(Debug, Clone, Eq, PartialEq, Default)]
pub struct SubscriptionsView {
    // All have the same peers (whole network) initially
    pub echo: HashSet<PeerId>,
    pub ready: HashSet<PeerId>,
    pub network_size: usize,
}

impl SubscriptionsView {
    pub fn is_some(&self) -> bool {
        !self.is_none()
    }

    pub fn is_none(&self) -> bool {
        self.echo.is_empty() && self.ready.is_empty()
    }

    /// Current view of subscriptions of the node, which is the whole network
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
