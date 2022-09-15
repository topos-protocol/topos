use std::time::Duration;
use tokio::sync::mpsc;
use tracing::{info, span, subscriber, Metadata, Subscriber};
use tracing_subscriber::{registry::LookupSpan, Layer};

use crate::{
    aggregator::Aggregator,
    builder::Builder,
    event::Event,
    server::Server,
    visitors::{double_echo::DoubleEchoVisitor, network::NetworkVisitor, runtime::RuntimeVisitor},
};

const P2P_RUNTIME_HANDLE_EVENT: &str = "topos_p2p::runtime::handle_event";
const TCE_RUNTIME_EVENT: &str = "topos_tce_api::runtime";
const TCE_DOUBLE_ECHO_EVENT: &str = "topos_tce_broadcast::double_echo";

pub struct ConsoleLayer {
    tx: mpsc::Sender<Event>,
}

impl ConsoleLayer {
    pub const DEFAULT_EVENT_BUFFER_CAPACITY: usize = 1024 * 100;
    pub const DEFAULT_CLIENT_BUFFER_CAPACITY: usize = 1024 * 4;
    pub const DEFAULT_PUBLISH_INTERVAL: Duration = Duration::from_secs(1);

    pub fn builder() -> Builder {
        Builder::default()
    }

    pub(crate) fn build(config: Builder) -> (Self, Server) {
        let (tx, events) = mpsc::channel(config.event_buffer_capacity);
        let (subscribe, rpcs) = mpsc::channel(256);
        // let shared = Arc::new(Shared::default());
        let aggregator = Aggregator::new(events, rpcs, &config);

        let server = Server {
            aggregator: Some(aggregator),
            addr: config.server_addr,
            subscribe,
            client_buffer: config.client_buffer_capacity,
        };

        let layer = Self { tx };

        (layer, server)
    }
}

impl<S> Layer<S> for ConsoleLayer
where
    S: Subscriber + for<'a> LookupSpan<'a>,
{
    fn register_callsite(&self, _meta: &'static Metadata<'static>) -> subscriber::Interest {
        subscriber::Interest::always()
    }

    fn on_new_span(
        &self,
        attrs: &span::Attributes<'_>,
        _id: &span::Id,
        _ctx: tracing_subscriber::layer::Context<'_, S>,
    ) {
        let metadata = attrs.metadata();

        match (metadata.name(), metadata.target()) {
            // (metadata, target) => {
            //     println!("on new span {:#?}, {:?}", metadata, target);
            // }
            _ => {}
        }
    }

    fn on_event(
        &self,
        event: &tracing::Event<'_>,
        _ctx: tracing_subscriber::layer::Context<'_, S>,
    ) {
        let metadata = event.metadata();

        match (metadata.name(), metadata.target()) {
            (_, TCE_RUNTIME_EVENT) => {
                let mut runtime_visitor = RuntimeVisitor::default();
                event.record(&mut runtime_visitor);

                if let Some(result) = runtime_visitor.result() {
                    let _ = self.tx.try_send(Event::Runtime(result));
                }
            }

            (_, TCE_DOUBLE_ECHO_EVENT) => {
                let mut double_echo_visitor = DoubleEchoVisitor::default();
                event.record(&mut double_echo_visitor);

                if let Some(result) = double_echo_visitor.result() {
                    let _ = self.tx.try_send(Event::DoubleEcho(result));
                }
            }

            (_, P2P_RUNTIME_HANDLE_EVENT) => {
                let mut network_visitor = NetworkVisitor::default();
                event.record(&mut network_visitor);

                if let Some(result) = network_visitor.result() {
                    info!("Sending event to aggregator");
                    let _ = self.tx.try_send(Event::Network(result));
                }
            }

            _ => {}
        }
    }

    fn on_enter(&self, _id: &span::Id, _ctx: tracing_subscriber::layer::Context<'_, S>) {}
}
