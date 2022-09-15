use std::{
    net::{SocketAddr, ToSocketAddrs},
    thread,
    time::Duration,
};

use tokio::runtime;
use tracing::{info, Subscriber};
use tracing_subscriber::{
    filter::{self, FilterFn},
    prelude::__tracing_subscriber_SubscriberExt,
    registry::LookupSpan,
    util::SubscriberInitExt,
    Layer,
};

use crate::{layer::ConsoleLayer, server::Server};

#[derive(Clone, Debug)]
pub struct Builder {
    pub(crate) server_addr: SocketAddr,
    pub(crate) filter_env_var: String,
    pub(crate) event_buffer_capacity: usize,
    pub(crate) client_buffer_capacity: usize,
    pub(crate) publish_interval: Duration,
}

impl Default for Builder {
    fn default() -> Self {
        Self {
            server_addr: SocketAddr::new(Server::DEFAULT_IP, Server::DEFAULT_PORT),
            filter_env_var: "RUST_LOG".to_string(),
            event_buffer_capacity: ConsoleLayer::DEFAULT_EVENT_BUFFER_CAPACITY,
            client_buffer_capacity: ConsoleLayer::DEFAULT_CLIENT_BUFFER_CAPACITY,
            publish_interval: ConsoleLayer::DEFAULT_PUBLISH_INTERVAL,
        }
    }
}

impl Builder {
    pub fn build(self) -> (ConsoleLayer, Server) {
        ConsoleLayer::build(self)
    }

    /// | **Environment Variable**       | **Purpose**                                                  | **Default Value** |
    /// |--------------------------------|--------------------------------------------------------------|-------------------|
    /// | `TCE_CONSOLE_BIND`             | a HOST:PORT description, such as `localhost:1234`            | `127.0.0.1:6669`  |
    pub fn with_default_env(mut self) -> Self {
        if let Ok(bind) = std::env::var("TCE_CONSOLE_BIND") {
            self.server_addr = bind
                .to_socket_addrs()
                .expect("TCE_CONSOLE_BIND must be formatted as HOST:PORT, such as localhost:4321")
                .next()
                .expect("TCE console could not resolve TCE_CONSOLE_BIND");
        }

        self
    }

    pub fn init(self) {
        type Filter = filter::EnvFilter;

        let fmt_filter = std::env::var(&self.filter_env_var)
            .ok()
            .and_then(|log_filter| match log_filter.parse::<Filter>() {
                Ok(targets) => Some(targets),
                Err(e) => {
                    eprintln!(
                        "failed to parse filter environment variable `{}={:?}`: {}",
                        &self.filter_env_var, log_filter, e
                    );
                    None
                }
            })
            .unwrap_or_else(|| {
                "error"
                    .parse::<Filter>()
                    .expect("`error` filter should always parse successfully")
            });

        let console_layer = self.spawn();

        tracing_subscriber::registry()
            .with(console_layer)
            .with(tracing_subscriber::fmt::layer().with_filter(fmt_filter))
            .init();
    }

    pub fn spawn<S>(self) -> impl Layer<S>
    where
        S: Subscriber + for<'a> LookupSpan<'a>,
    {
        fn console_filter(meta: &tracing::Metadata<'_>) -> bool {
            // events will have *targets* beginning with "runtime"
            if meta.is_event() {
                return meta.target().starts_with("topos_p2p")
                    || meta.target().starts_with("topos_tce");
            }

            // spans will have *names* beginning with "runtime". for backwards
            // compatibility with older Tokio versions, enable anything with the `tokio`
            // target as well.
            meta.name().starts_with("topos_core") || meta.target().starts_with("topos_tce")
        }

        let (layer, server) = self.build();
        let filter =
            FilterFn::new(console_filter as for<'r, 's> fn(&'r tracing::Metadata<'s>) -> bool);
        let layer = layer.with_filter(filter);

        thread::Builder::new()
            .name("console_subscriber".into())
            .spawn(move || {
                // let _subscriber_guard;
                // if !self_trace {
                //     _subscriber_guard = tracing::subscriber::set_default(
                //         tracing_core::subscriber::NoSubscriber::default(),
                //     );
                // }
                let runtime = runtime::Builder::new_current_thread()
                    .enable_io()
                    .enable_time()
                    .build()
                    .expect("console subscriber runtime initialization failed");

                runtime.block_on(async move {
                    info!("Starting console server");
                    server
                        .serve()
                        .await
                        .expect("console subscriber server failed")
                });
            })
            .expect("console subscriber could not spawn thread");

        layer
    }
}
