use futures::{FutureExt, Stream as FutureStream};
use opentelemetry::{
    trace::{FutureExt as TraceFutureExt, TraceContextExt},
    Context,
};
use std::pin::Pin;
use tokio::sync::{mpsc, oneshot};
use tokio_stream::wrappers::ReceiverStream;
use tonic::{Request, Response, Status, Streaming};
use topos_core::api::tce::v1::{
    api_service_server::ApiService, GetSourceHeadRequest, GetSourceHeadResponse,
    SubmitCertificateRequest, SubmitCertificateResponse, WatchCertificatesRequest,
    WatchCertificatesResponse,
};
use tracing::{error, field, info, instrument, Instrument, Span};
use tracing_opentelemetry::OpenTelemetrySpanExt;

use crate::runtime::InternalRuntimeCommand;

pub(crate) mod console;
#[cfg(test)]
mod tests;

const DEFAULT_CHANNEL_STREAM_CAPACITY: usize = 100;

pub(crate) mod builder;

#[derive(Debug)]
pub(crate) struct TceGrpcService {
    local_peer_id: String,
    command_sender: mpsc::Sender<InternalRuntimeCommand>,
}

#[tonic::async_trait]
impl ApiService for TceGrpcService {
    #[instrument(name = "CertificateSubmitted", skip(self, request), fields(peer_id = self.local_peer_id, certificate_id = field::Empty))]
    async fn submit_certificate(
        &self,
        request: Request<SubmitCertificateRequest>,
    ) -> Result<Response<SubmitCertificateResponse>, Status> {
        tracing::warn!(span_span_id = ?Span::current().context().span().span_context().span_id(), "pre_run");
        tracing::warn!(cx_span_id = ?Context::current().span().span_context().span_id(), "pre_run");

        // let parent_cx =
        //     global::get_text_map_propagator(|prop| prop.extract(&MetadataMap(request.metadata())));
        // tracing::Span::current().set_parent(parent_cx);
        let data = request.into_inner();
        if let Some(certificate) = data.certificate {
            if let Some(ref id) = certificate.id {
                Span::current().record("certificate_id", id.to_string());

                let (sender, receiver) = oneshot::channel();

                if self
                    .command_sender
                    .send(InternalRuntimeCommand::CertificateSubmitted {
                        certificate: Box::new(certificate.into()),
                        sender,
                        ctx: Span::current().context(),
                    })
                    .with_current_context()
                    .instrument(Span::current())
                    .await
                    .is_err()
                {
                    return Err(Status::internal("Can't submit certificate: sender dropped"));
                }

                receiver
                    .map(|value| match value {
                        Ok(Ok(_)) => Ok(Response::new(SubmitCertificateResponse {})),
                        Ok(Err(_)) => Err(Status::internal("Can't submit certificate")),
                        Err(_) => Err(Status::internal("Can't submit certificate")),
                    })
                    .await
            } else {
                error!("No certificate id provided");
                Err(Status::invalid_argument("Certificate is malformed"))
            }
        } else {
            Err(Status::invalid_argument("Certificate is malformed"))
        }
    }

    async fn get_source_head(
        &self,
        request: Request<GetSourceHeadRequest>,
    ) -> Result<Response<GetSourceHeadResponse>, Status> {
        let data = request.into_inner();
        if let Some(subnet_id) = data.subnet_id {
            let (sender, receiver) =
                oneshot::channel::<Result<(u64, topos_core::uci::Certificate), _>>();

            if self
                .command_sender
                .send(InternalRuntimeCommand::GetSourceHead {
                    subnet_id: subnet_id.into(),
                    sender,
                })
                .await
                .is_err()
            {
                return Err(Status::internal(
                    "Can't get delivered certificate position by source: sender dropped",
                ));
            }

            receiver
                .map(|value| match value {
                    Ok(Ok((position, certificate))) => Ok(Response::new(GetSourceHeadResponse {
                        certificate: Some(certificate.clone().into()),
                        position: Some(topos_core::api::tce::v1::SourceStreamPosition {
                            subnet_id: Some(certificate.source_subnet_id.into()),
                            certificate_id: Some((*certificate.id.as_array()).into()),
                            position,
                        }),
                    })),
                    Ok(Err(crate::RuntimeError::UnknownSubnet(subnet_id))) => {
                        // Tce does not have Position::Zero certificate associated
                        Err(Status::internal(format!(
                            "Unknown subnet, no genesis certificate associated with subnet id {:?}",
                            &subnet_id
                        )))
                    }
                    Ok(Err(_)) => Err(Status::internal(
                        "Can't get source head certificate position",
                    )),
                    Err(_) => Err(Status::internal(
                        "Can't get source head certificate position",
                    )),
                })
                .await
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

        if self
            .command_sender
            .send(InternalRuntimeCommand::NewStream {
                stream,
                sender,
                internal_runtime_command_sender: self.command_sender.clone(),
            })
            .await
            .is_err()
        {
            return Err(Status::internal("Can't submit certificate: sender dropped"));
        }

        Ok(Response::new(
            Box::pin(ReceiverStream::new(rx)) as Self::WatchCertificatesStream
        ))
    }
}
