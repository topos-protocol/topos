use tonic::Status;
use topos_core::api::grpc::checkpoints::{TargetCheckpoint, TargetStreamPosition};
use topos_core::api::grpc::tce::v1::watch_certificates_request::Command;
use topos_core::api::grpc::tce::v1::watch_certificates_request::OpenStream as GrpcOpenStream;
use topos_core::api::grpc::tce::v1::watch_certificates_response::CertificatePushed as GrpcCertificatePushed;
use topos_core::api::grpc::tce::v1::watch_certificates_response::Event;
use topos_core::api::grpc::tce::v1::watch_certificates_response::StreamOpened as GrpcStreamOpened;
use topos_core::types::CertificateDelivered;
use topos_core::uci::SubnetId;

pub enum InboundMessage {
    OpenStream(OpenStream),
}

pub struct OpenStream {
    pub(crate) target_checkpoint: TargetCheckpoint,
}

#[derive(Debug)]
pub struct CertificatePushed {
    pub(crate) certificate: CertificateDelivered,
    pub(crate) positions: Vec<TargetStreamPosition>,
}

#[derive(Debug)]
pub enum OutboundMessage {
    StreamOpened(StreamOpened),
    CertificatePushed(Box<CertificatePushed>),
}

#[derive(Debug)]
pub struct StreamOpened {
    pub(crate) subnet_ids: Vec<SubnetId>,
}

impl TryFrom<Command> for InboundMessage {
    type Error = Status;

    fn try_from(command: Command) -> Result<Self, Self::Error> {
        match command {
            Command::OpenStream(value) => Ok(OpenStream::try_from(value)?.into()),
        }
    }
}

impl TryFrom<GrpcOpenStream> for OpenStream {
    type Error = Status;

    fn try_from(value: GrpcOpenStream) -> Result<Self, Self::Error> {
        Ok(Self {
            target_checkpoint: value.target_checkpoint.map(TryInto::try_into).map_or(
                Err(Status::invalid_argument("missing target_checkpoint")),
                |value| value.map_err(|_| Status::invalid_argument("invalid checkpoint")),
            )?,
        })
    }
}

impl From<OpenStream> for InboundMessage {
    fn from(value: OpenStream) -> Self {
        Self::OpenStream(value)
    }
}

impl From<OutboundMessage> for Event {
    fn from(value: OutboundMessage) -> Self {
        match value {
            OutboundMessage::StreamOpened(StreamOpened { subnet_ids }) => {
                Self::StreamOpened(GrpcStreamOpened {
                    subnet_ids: subnet_ids.into_iter().map(Into::into).collect(),
                })
            }
            OutboundMessage::CertificatePushed(certificate_pushed) => {
                Self::CertificatePushed(GrpcCertificatePushed {
                    certificate: Some(certificate_pushed.certificate.certificate.into()),
                    positions: certificate_pushed
                        .positions
                        .into_iter()
                        .map(Into::into)
                        .collect(),
                })
            }
        }
    }
}
