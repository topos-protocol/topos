pub mod aggregator;
mod cyclerng;
mod sampling;

// Move to transport crate
pub type Peer = String;
use std::collections::{HashMap, HashSet};

#[derive(Debug, Default)]
pub struct ThresholdConfig {
    pub echo: usize,
    pub ready: usize,
    pub delivery: usize,
}

#[derive(Debug)]
pub enum SampleProviderStatus {
    /// The view on the peers changed hence building new samples
    BuildingNewView,
    /// The last request for sample build is done
    Stabilized,
}

#[derive(PartialEq, Eq, Hash, Debug, Clone)]
pub enum SampleType {
    /// Inbound: FROM external peer TO me
    /// Message from those I am following
    EchoInbound,
    ReadyInbound,
    DeliveryInbound,

    /// Outbound: FROM me TO external peer
    /// Message to my followers
    EchoOutbound,
    ReadyOutbound,
}

//#[derive(Debug, Default)]
pub type SampleView = HashMap<SampleType, HashSet<Peer>>;
