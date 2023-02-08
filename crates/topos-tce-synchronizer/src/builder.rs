use std::future::IntoFuture;

use futures::{future::BoxFuture, FutureExt, TryFutureExt};
use tokio::{spawn, sync::mpsc, sync::oneshot};
use tokio_stream::wrappers::ReceiverStream;
use topos_p2p::Client as NetworkClient;
use topos_tce_gatekeeper::GatekeeperClient;

use crate::{
    checkpoints_collector::CheckpointsCollector, client::SynchronizerClient, Synchronizer,
    SynchronizerError, SynchronizerEvent,
};

#[derive(Default)]
pub struct SynchronizerBuilder {
    gatekeeper_client: Option<GatekeeperClient>,
    network_client: Option<NetworkClient>,
}

impl IntoFuture for SynchronizerBuilder {
    type Output = Result<
        (
            SynchronizerClient,
            Synchronizer,
            ReceiverStream<SynchronizerEvent>,
        ),
        SynchronizerError,
    >;

    type IntoFuture = BoxFuture<'static, Self::Output>;

    fn into_future(mut self) -> Self::IntoFuture {
        let (shutdown_channel, shutdown) = mpsc::channel::<oneshot::Sender<()>>(1);
        let (commands, commands_recv) = mpsc::channel(100);
        let (events, events_recv) = mpsc::channel(100);

        CheckpointsCollector::builder()
            .set_gatekeeper_client(self.gatekeeper_client.take())
            .set_network_client(self.network_client.take())
            .into_future()
            .map_err(Into::into)
            .and_then(
                |(checkpoints_collector, runtime, checkpoints_collector_stream)| {
                    spawn(runtime.into_future());

                    futures::future::ok((
                        SynchronizerClient {
                            shutdown_channel,
                            commands,
                        },
                        Synchronizer {
                            shutdown,
                            commands: commands_recv,
                            events,
                            checkpoints_collector,
                            checkpoints_collector_stream,
                        },
                        ReceiverStream::new(events_recv),
                    ))
                },
            )
            .boxed()
    }
}

impl SynchronizerBuilder {
    pub fn with_gatekeeper_client(mut self, gatekeeper_client: GatekeeperClient) -> Self {
        self.gatekeeper_client = Some(gatekeeper_client);

        self
    }

    pub fn with_network_client(mut self, network_client: NetworkClient) -> Self {
        self.network_client = Some(network_client);

        self
    }
}
