use std::collections::HashSet;
use topos_p2p::PeerId;

// These are all the same in the new, deterministic view (Subscriptions == Subscribers)
#[derive(PartialEq, Eq, Hash, Debug, Clone)]
pub enum SampleType {
    /// Inbound: FROM external peer TO me
    /// Message from those I am following
    EchoSubscription,
    ReadySubscription,
    DeliverySubscription,

    /// Outbound: FROM me TO external peer
    /// Message to my followers
    EchoSubscriber,
    ReadySubscriber,
}

// Need to separate echo and ready (echo removes it from echo set, ready removes it from ready and delivery set)
// TODO: HashSet is all participants, once I receive echo | ready | delivery, I remove it to get to the threshold
// Maybe structure for keeping track of different counters
#[derive(Debug, Clone, Eq, PartialEq, Default)]
pub struct SubscriptionsView {
    // All have the same peers (whole network) initially
    pub echo: HashSet<PeerId>,
    pub ready: HashSet<PeerId>,
    pub delivery: HashSet<PeerId>,
    pub network_size: usize,
}

impl SubscriptionsView {
    pub fn is_some(&self) -> bool {
        !self.is_none()
    }

    pub fn is_none(&self) -> bool {
        self.echo.is_empty() && self.ready.is_empty() && self.delivery.is_empty()
    }

    /// Current view of subscriptions of the node, which is the whole network
    pub fn get_subscriptions(&self) -> Vec<PeerId> {
        self.echo
            .iter()
            .chain(self.ready.iter())
            .chain(self.delivery.iter())
            .cloned()
            .collect::<HashSet<_>>()
            .into_iter()
            .collect()
    }
}
