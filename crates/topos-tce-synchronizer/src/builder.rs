use std::future::IntoFuture;

use futures::{future::BoxFuture, FutureExt};
use tokio::sync::mpsc;
use tokio_stream::wrappers::ReceiverStream;

use crate::{client::SynchronizerClient, Synchronizer, SynchronizerError, SynchronizerEvent};

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

        futures::future::ok((
            SynchronizerClient {
                shutdown_channel,
                commands,
            },
            Synchronizer {
                shutdown,
                commands: commands_recv,
                events,
            },
            ReceiverStream::new(events_recv),
        ))
        .boxed()
    }
}
