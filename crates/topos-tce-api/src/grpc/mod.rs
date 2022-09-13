use futures::Stream as FutureStream;
use std::pin::Pin;
use tokio::sync::mpsc;
use tokio_stream::wrappers::ReceiverStream;
use tonic::{Request, Response, Status, Streaming};
use topos_core::api::tce::v1::{
    api_service_server::ApiService, SubmitCertificateRequest, SubmitCertificateResponse,
    WatchCertificatesRequest, WatchCertificatesResponse,
};
use tracing::info;

use crate::runtime::InternalRuntimeCommand;

#[cfg(test)]
mod tests;

const DEFAULT_CHANNEL_STREAM_CAPACITY: usize = 100;

pub(crate) mod builder;

#[derive(Debug)]
pub(crate) struct TceGrpcService {
    command_sender: mpsc::Sender<InternalRuntimeCommand>,
}

#[tonic::async_trait]
impl ApiService for TceGrpcService {
    async fn submit_certificate(
        &self,
        request: Request<SubmitCertificateRequest>,
    ) -> Result<Response<SubmitCertificateResponse>, Status> {
        let data = request.into_inner();
        if let Some(certificate) = data.certificate {
            _ = self
                .command_sender
                .send(InternalRuntimeCommand::CertificateSubmitted {
                    certificate: certificate.into(),
                })
                .await;
            Ok(Response::new(SubmitCertificateResponse {}))
        } else {
            Err(Status::invalid_argument("Certificate is malformed"))
        }
    }

    ///Server streaming response type for the WatchCertificates method.
    type WatchCertificatesStream = Pin<
        Box<dyn FutureStream<Item = Result<WatchCertificatesResponse, Status>> + Send + 'static>,
    >;

    /// This RPC allows a client to open a bidirectional stream with a TCE
    async fn watch_certificates(
        &self,
        request: Request<Streaming<WatchCertificatesRequest>>,
    ) -> Result<Response<Self::WatchCertificatesStream>, Status> {
        match request.remote_addr() {
            Some(addr) => info!(client.addr = %addr, "starting a new stream"),
            None => info!(client.addr = %"<unknown>", "starting a new stream"),
        }

        let stream: Streaming<_> = request.into_inner();

        let (sender, rx) = mpsc::channel::<Result<WatchCertificatesResponse, Status>>(
            DEFAULT_CHANNEL_STREAM_CAPACITY,
        );

        _ = self
            .command_sender
            .send(InternalRuntimeCommand::NewStream {
                stream,
                sender,
                internal_runtime_command_sender: self.command_sender.clone(),
            })
            .await;

        Ok(Response::new(
            Box::pin(ReceiverStream::new(rx)) as Self::WatchCertificatesStream
        ))
    }
}
