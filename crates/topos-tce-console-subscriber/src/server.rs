use std::net::{IpAddr, Ipv4Addr, SocketAddr};

use tokio::{spawn, sync::mpsc};

use crate::{
    aggregator::Aggregator,
    command::{Command, Watch},
};

use topos_tce_console_api as proto;

pub struct Server {
    pub(crate) subscribe: mpsc::Sender<Command>,
    pub(crate) addr: SocketAddr,
    pub(crate) aggregator: Option<Aggregator>,
    pub(crate) client_buffer: usize,
}

impl Server {
    pub const DEFAULT_IP: IpAddr = IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1));
    pub const DEFAULT_PORT: u16 = 6669;

    pub async fn serve(self) -> Result<(), Box<dyn std::error::Error + Send + Sync + 'static>> {
        self.serve_with(tonic::transport::Server::default()).await
    }

    pub async fn serve_with(
        mut self,
        mut builder: tonic::transport::Server,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync + 'static>> {
        let aggregate = self
            .aggregator
            .take()
            .expect("cannot start server multiple times");
        let aggregate = spawn(aggregate.run());
        let addr = self.addr;
        let serve = builder
            .add_service(proto::instrument::instrument_server::InstrumentServer::new(
                self,
            ))
            .serve(addr);
        let res = spawn(serve).await;

        aggregate.abort();

        res?.map_err(Into::into)
    }
}

#[tonic::async_trait]
impl proto::instrument::instrument_server::Instrument for Server {
    type WatchUpdatesStream = tokio_stream::wrappers::ReceiverStream<
        Result<proto::instrument::WatchUpdatesResponse, tonic::Status>,
    >;

    async fn watch_updates(
        &self,
        req: tonic::Request<proto::instrument::WatchUpdatesRequest>,
    ) -> Result<tonic::Response<Self::WatchUpdatesStream>, tonic::Status> {
        match req.remote_addr() {
            Some(addr) => tracing::debug!(client.addr = %addr, "starting a new watch"),
            None => tracing::debug!(client.addr = %"<unknown>", "starting a new watch"),
        }
        let permit = self.subscribe.reserve().await.map_err(|_| {
            tonic::Status::internal("cannot start new watch, aggregation task is not running")
        })?;
        let (tx, rx) = mpsc::channel(self.client_buffer);
        permit.send(Command::Instrument(Watch(tx)));
        tracing::debug!("watch started");
        let stream = tokio_stream::wrappers::ReceiverStream::new(rx);
        Ok(tonic::Response::new(stream))
    }
}
