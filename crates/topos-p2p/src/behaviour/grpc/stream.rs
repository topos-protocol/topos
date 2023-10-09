use std::{
    io,
    pin::Pin,
    sync::{Arc, Mutex},
    task::{Context, Poll},
};

use futures::{AsyncRead as FuturesAsyncRead, AsyncWrite as FuturesAsyncWrite, Future};
use http::Uri;
use libp2p::{swarm::ConnectionId, PeerId};
use pin_project::pin_project;
use tokio::{
    io::{AsyncRead, AsyncWrite, ReadBuf},
    sync::mpsc,
};
use tonic::transport::{server::Connected, Channel, Endpoint};
use tower::{BoxError, Service};
use tracing::info;

/// Manage a gRPC Stream linked to an open [`libp2p::Stream`]
#[pin_project]
pub(crate) struct GrpcStream {
    #[pin]
    stream: libp2p::Stream,
    peer_id: PeerId,
    connection_id: libp2p::swarm::ConnectionId,
}

/// Outbound GrpcStream initialization struct
#[pin_project]
struct InitializedGrpcOutboundStream {
    #[pin]
    stream_rx: Arc<Mutex<mpsc::Receiver<Result<GrpcStream, BoxError>>>>,
}

/// Fully negotiated Outbound GrpcStream
#[pin_project]
struct NegotiatedGrpcOutboundStream {
    #[pin]
    stream_rx: Arc<Mutex<mpsc::Receiver<Result<GrpcStream, BoxError>>>>,
}

impl Future for NegotiatedGrpcOutboundStream {
    type Output = Result<GrpcStream, BoxError>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let fut = self.project();
        let x = fut
            .stream_rx
            .lock()
            .unwrap()
            .poll_recv(cx)
            .map(|option| option.unwrap());

        x
    }
}

impl Service<Uri> for InitializedGrpcOutboundStream {
    type Response = GrpcStream;

    type Error = BoxError;

    type Future = NegotiatedGrpcOutboundStream;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        Poll::Ready(Ok(()))
    }

    fn call(&mut self, req: Uri) -> Self::Future {
        NegotiatedGrpcOutboundStream {
            stream_rx: self.stream_rx.clone(),
        }
    }
}

impl GrpcStream {
    pub fn new(stream: libp2p::Stream, peer_id: PeerId, connection_id: ConnectionId) -> Self {
        Self {
            stream,
            peer_id,
            connection_id,
        }
    }

    /// Transform the GrpcStream into a [`tonic::transport::Channel`]
    pub async fn into_channel(self) -> Result<Channel, tonic::transport::Error> {
        let (sender, receiver) = mpsc::channel(1);

        let connection = InitializedGrpcOutboundStream {
            stream_rx: Arc::new(Mutex::new(receiver)),
        };

        let fut = async move {
            Endpoint::try_from("http://[::]:50051")
                .unwrap()
                .connect_with_connector(connection)
                .await
        };

        let (channel, send_result) = tokio::join!(fut, sender.send(Ok(self)));

        channel
    }
}

impl Connected for GrpcStream {
    type ConnectInfo = ();

    fn connect_info(&self) -> Self::ConnectInfo {}
}

impl AsyncRead for GrpcStream {
    fn poll_read(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<io::Result<()>> {
        let unfilled = buf.initialize_unfilled();

        self.project()
            .stream
            .poll_read(cx, unfilled)
            .map_ok(|len| buf.advance(len))
    }
}

impl AsyncWrite for GrpcStream {
    fn poll_write(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<Result<usize, io::Error>> {
        self.project().stream.poll_write(cx, buf)
    }

    fn poll_flush(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), io::Error>> {
        self.project().stream.poll_flush(cx)
    }

    fn poll_shutdown(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), io::Error>> {
        self.project().stream.poll_close(cx)
    }
}
