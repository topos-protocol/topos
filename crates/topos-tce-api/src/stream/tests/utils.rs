use hyper::body::Sender;
use prost::Message;
use tokio::sync::mpsc;
use tonic::{
    codec::{Codec, ProstCodec},
    transport::Body,
    Status, Streaming,
};
use topos_core::api::tce::v1::WatchCertificatesResponse;
use uuid::Uuid;

use crate::{
    runtime::InternalRuntimeCommand,
    stream::{Stream, StreamCommand},
};

pub fn create_stream<M: Message + Default + 'static>() -> (Sender, Streaming<M>) {
    let (tx, body) = Body::channel();
    let mut codec = ProstCodec::<M, M>::default();
    // Note: This is an undocumented function.
    let stream = Streaming::new_request(codec.decoder(), body, None);

    (tx, stream)
}

pub struct StreamBuilder {
    outbound_stream_channel_size: usize,
    runtime_channel_size: usize,
    stream_channel_size: usize,
    #[allow(dead_code)]
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
        let (tx, stream) = create_stream();
        let (sender, stream_receiver) = mpsc::channel(self.outbound_stream_channel_size);
        let (command_sender, command_receiver) = mpsc::channel(self.stream_channel_size);
        let (internal_runtime_command_sender, runtime_receiver) =
            mpsc::channel(self.runtime_channel_size);
        let stream_id = Uuid::new_v4();

        let testable_stream = Stream {
            stream_id,
            sender,
            internal_runtime_command_sender,
            stream,
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
    pub(crate) stream_receiver: mpsc::Receiver<Result<WatchCertificatesResponse, Status>>,
    #[allow(dead_code)]
    pub(crate) command_sender: mpsc::Sender<StreamCommand>,
    pub(crate) runtime_receiver: mpsc::Receiver<InternalRuntimeCommand>,
    pub(crate) stream_id: Uuid,
}
