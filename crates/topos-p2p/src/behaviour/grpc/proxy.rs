use std::{
    io,
    pin::Pin,
    task::{Context, Poll},
};

use futures::Stream;
use pin_project::pin_project;
use tokio::sync::mpsc;

use super::stream::GrpcStream;

/// Proxy for gRPC connection with the local service.
#[pin_project]
pub(crate) struct GrpcProxy {
    #[pin]
    rx: mpsc::UnboundedReceiver<io::Result<GrpcStream>>,
}

impl GrpcProxy {
    pub(crate) fn new(rx: mpsc::UnboundedReceiver<io::Result<GrpcStream>>) -> Self {
        Self { rx }
    }
}

impl Stream for GrpcProxy {
    type Item = io::Result<GrpcStream>;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        self.project().rx.as_mut().poll_recv(cx)
    }
}
