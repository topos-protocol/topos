use std::future::IntoFuture;

use futures::{future::BoxFuture, FutureExt};
use tokio::sync::mpsc;
use tokio_stream::wrappers::ReceiverStream;

use super::{
    CheckpointsCollector, CheckpointsCollectorClient, CheckpointsCollectorError,
    CheckpointsCollectorEvent,
};

pub struct CheckpointsCollectorBuilder {}

impl IntoFuture for CheckpointsCollectorBuilder {
    type Output = Result<
        (
            CheckpointsCollectorClient,
            CheckpointsCollector,
            ReceiverStream<CheckpointsCollectorEvent>,
        ),
        CheckpointsCollectorError,
    >;

    type IntoFuture = BoxFuture<'static, Self::Output>;

    fn into_future(self) -> Self::IntoFuture {
        let (shutdown_channel, shutdown) = mpsc::channel(1);
        let (commands, commands_recv) = mpsc::channel(100);
        let (events, events_recv) = mpsc::channel(100);

        futures::future::ok((
            CheckpointsCollectorClient {
                shutdown_channel,
                commands,
            },
            CheckpointsCollector {
                shutdown,
                commands: commands_recv,
                events,
            },
            ReceiverStream::new(events_recv),
        ))
        .boxed()
    }
}
