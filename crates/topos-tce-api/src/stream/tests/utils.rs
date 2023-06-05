use std::collections::HashMap;

use futures::{stream::BoxStream, StreamExt};
use hyper::body::Sender;
use tokio::sync::mpsc;
use tonic::{
    codec::{Codec, ProstCodec},
    transport::Body,
    Status, Streaming,
};
use topos_core::api::grpc::tce::v1::{WatchCertificatesRequest, WatchCertificatesResponse};
use uuid::Uuid;

use crate::{
    grpc::{
        messaging::{InboundMessage, OutboundMessage},
        TceGrpcService,
    },
    runtime::InternalRuntimeCommand,
    stream::{Stream, StreamCommand, StreamError},
};

type CreateStreamResult = (
    Sender,
    BoxStream<'static, Result<(Option<Uuid>, InboundMessage), StreamError>>,
);

pub fn create_stream(stream_id: Uuid) -> CreateStreamResult {
    let (tx, body) = Body::channel();
    let mut codec = ProstCodec::<WatchCertificatesResponse, WatchCertificatesRequest>::default();
    let stream = Streaming::new_request(codec.decoder(), body, None)
        .map(move |message| TceGrpcService::parse_stream(message, stream_id))
        .boxed();

    (tx, stream)
}

pub struct StreamBuilder {
    outbound_stream_channel_size: usize,
    runtime_channel_size: usize,
    stream_channel_size: usize,
    stream_id: Uuid,
}

impl Default for StreamBuilder {
    fn default() -> Self {
        Self {
            outbound_stream_channel_size: 10,
            runtime_channel_size: 10,
            stream_channel_size: 10,
            stream_id: Uuid::new_v4(),
        }
    }
}

impl StreamBuilder {
    #[allow(dead_code)]
    pub fn outbound_stream_channel_size(mut self, value: usize) -> Self {
        self.outbound_stream_channel_size = value;

        self
    }

    #[allow(dead_code)]
    pub fn runtime_channel_size(mut self, value: usize) -> Self {
        self.runtime_channel_size = value;

        self
    }

    #[allow(dead_code)]
    pub fn stream_channel_size(mut self, value: usize) -> Self {
        self.stream_channel_size = value;

        self
    }

    #[allow(dead_code)]
    pub fn stream_id(mut self, value: Uuid) -> Self {
        self.stream_id = value;

        self
    }

    pub fn build(self) -> (Sender, Stream, StreamContext) {
        let stream_id = Uuid::new_v4();
        let (tx, stream) = create_stream(stream_id);
        let (sender, stream_receiver) = mpsc::channel(self.outbound_stream_channel_size);
        let (command_sender, command_receiver) = mpsc::channel(self.stream_channel_size);
        let (internal_runtime_command_sender, runtime_receiver) =
            mpsc::channel(self.runtime_channel_size);

        let testable_stream = Stream {
            stream_id,
            target_subnet_listeners: HashMap::new(),
            outbound_stream: sender,
            inbound_stream: stream,
            internal_runtime_command_sender,
            command_receiver,
        };

        (
            tx,
            testable_stream,
            StreamContext {
                stream_receiver,
                command_sender,
                runtime_receiver,
                stream_id,
            },
        )
    }
}

pub struct StreamContext {
    pub(crate) stream_receiver: mpsc::Receiver<Result<(Option<Uuid>, OutboundMessage), Status>>,
    #[allow(dead_code)]
    pub(crate) command_sender: mpsc::Sender<StreamCommand>,
    pub(crate) runtime_receiver: mpsc::Receiver<InternalRuntimeCommand>,
    pub(crate) stream_id: Uuid,
}
