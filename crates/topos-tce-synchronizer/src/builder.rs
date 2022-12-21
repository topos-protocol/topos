use std::future::IntoFuture;

use futures::{future::BoxFuture, FutureExt, TryFutureExt};
use tokio::{spawn, sync::mpsc};
use tokio_stream::wrappers::ReceiverStream;

use crate::{
    checkpoints_collector::CheckpointsCollector, client::SynchronizerClient, Synchronizer,
    SynchronizerError, SynchronizerEvent,
};

pub struct SynchronizerBuilder {}

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

    fn into_future(self) -> Self::IntoFuture {
        let (shutdown_channel, shutdown) = mpsc::channel(1);
        let (commands, commands_recv) = mpsc::channel(100);
        let (events, events_recv) = mpsc::channel(100);

        CheckpointsCollector::builder()
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
