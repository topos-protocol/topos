use futures::FutureExt;
use proto::instrument::{
    ConnectedPeer, InitialState, PendingCertificate, Sample, Stream, ValidatedCertificate,
};
use std::{
    collections::HashMap,
    time::{Duration, SystemTime},
};
use tokio::sync::mpsc;
use topos_tce_console_api as proto;
use tracing::{debug, info};

use crate::{
    builder::Builder,
    command::{Command, Watch},
    event::Event,
    visitors::{double_echo::DoubleEchoEvent, network::NetworkEvent, runtime::RuntimeEvent},
};

#[derive(Default)]
struct State {
    current_view: HashMap<String, Vec<String>>,
    pending_certificates: Vec<PendingCertificate>,
    validated_certificates: Vec<ValidatedCertificate>,
    connected_peers: HashMap<String, ConnectedPeer>,
    streams: HashMap<String, Vec<String>>,
}
impl State {
    fn to_proto(&self) -> InitialState {
        InitialState {
            peers: self
                .connected_peers
                .iter()
                .map(|(_, peer)| peer.clone())
                .collect(),
            pending_certificate: self.pending_certificates.clone(),
            validated_certificate: self.validated_certificates.clone(),
            current_sample: self.create_sample(),
            streams: self.create_streams(),
        }
    }

    fn create_streams(&self) -> Vec<Stream> {
        let mut streams = Vec::new();

        for (stream_uuid, subnet_ids) in self.streams.clone() {
            streams.push(Stream {
                stream_uuid,
                subnet_ids,
            });
        }

        streams
    }

    fn create_sample(&self) -> Option<Sample> {
        let echo_subscribers = self.current_view.get("EchoSubscribers")?.clone();
        let ready_subscribers = self.current_view.get("ReadySubscribers")?.clone();
        let echo_subsriptions = self.current_view.get("EchoSubscriptions")?.clone();
        let ready_subsriptions = self.current_view.get("ReadySubscriptions")?.clone();
        let delivery_subsriptions = self.current_view.get("DeliverySubscriptions")?.clone();

        Some(Sample {
            echo_subscribers,
            ready_subscribers,
            echo_subsriptions,
            ready_subsriptions,
            delivery_subsriptions,
        })
    }
}

pub struct Aggregator {
    publish_interval: Duration,
    rpcs: mpsc::Receiver<Command>,
    events: mpsc::Receiver<Event>,
    pending_network_events: Vec<NetworkEvent>,
    pending_runtime_events: Vec<RuntimeEvent>,
    pending_double_echo_events: Vec<DoubleEchoEvent>,
    watchers: Vec<Watch>,
    state: State,
}

impl Aggregator {
    pub(crate) async fn run(mut self) {
        let mut publish = tokio::time::interval(self.publish_interval);
        loop {
            let should_send = tokio::select! {
                // if the flush interval elapses, flush data to the client
                _ = publish.tick() => {
                    true
                }

                // a new command from a client
                cmd = self.rpcs.recv() => {
                    match cmd {
                        Some(Command::Instrument(subscription)) => {
                            self.add_instrument_subscription(subscription);
                        },

                        None => {
                            debug!("rpc channel closed, terminating");
                            return;
                        }
                    };

                    false
                }

            };

            while let Some(event) = self.events.recv().now_or_never() {
                match event {
                    Some(event) => {
                        match event {
                            Event::DoubleEcho(event) => self.pending_double_echo_events.push(event),

                            Event::Network(network_event) => {
                                info!("Received network event");
                                match &network_event {
                                    NetworkEvent::PeerConnection {
                                        peer_id,
                                        direction,
                                        addresses,
                                    } => {
                                        _ = self.state.connected_peers.insert(
                                            peer_id.clone(),
                                            ConnectedPeer {
                                                peer_id: peer_id.clone(),
                                                addresses: addresses.clone(),
                                                direction: direction.clone(),
                                            },
                                        );
                                    }

                                    NetworkEvent::PeerDisconnection { peer_id, .. } => {
                                        _ = self.state.connected_peers.remove(peer_id);
                                    }
                                }
                                self.pending_network_events.push(network_event);
                            }
                            Event::Runtime(runtime_event) => {
                                self.pending_runtime_events.push(runtime_event);
                            }
                        }

                        // self.update_state(event);
                    }

                    None => {
                        debug!("event channel closed; terminating");
                        return;
                    }
                };
            }

            if !self.watchers.is_empty() && should_send {
                self.publish();
            }
        }
    }

    fn publish(&mut self) {
        let update = proto::instrument::WatchUpdatesResponse {
            now: Some(SystemTime::now().into()),
            initial_state: None,
            double_echo_update: self
                .pending_double_echo_events
                .iter()
                .map(|event| event.to_proto())
                .collect(),
            runtime_update: self
                .pending_runtime_events
                .iter()
                .map(|event| event.to_proto())
                .collect(),
            network_update: self
                .pending_network_events
                .iter()
                .map(|event| event.to_proto())
                .collect(),
        };

        self.watchers.retain(|watch: &Watch| watch.update(&update));
    }

    pub(crate) fn new(
        events: mpsc::Receiver<Event>,
        rpcs: mpsc::Receiver<Command>,
        builder: &Builder,
    ) -> Aggregator {
        Self {
            rpcs,
            publish_interval: builder.publish_interval,
            events,
            pending_network_events: Vec::new(),
            pending_runtime_events: Vec::new(),
            pending_double_echo_events: Vec::new(),
            watchers: Default::default(),
            state: State::default(),
        }
    }

    fn add_instrument_subscription(&mut self, subscription: Watch) {
        debug!("new instrument subscription");

        let update = &proto::instrument::WatchUpdatesResponse {
            network_update: Vec::new(),
            double_echo_update: Vec::new(),
            runtime_update: Vec::new(),
            initial_state: Some(self.state.to_proto()),
            now: Some(SystemTime::now().into()),
        };

        info!("Sending initial state: {update:?}");

        subscription.update(update);

        // Send the initial state --- if this fails, the subscription is already dead
        // if subscription.update(update) {
        self.watchers.push(subscription)
        // }
    }
}
