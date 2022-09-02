use std::time::Duration;

use super::Stream;
use bytes::{BufMut, Bytes, BytesMut};
use prost::Message;
use test_log::test;
use tokio::spawn;
use tokio::sync::mpsc;
use tonic::Streaming;
use tonic::{
    codec::{Codec, ProstCodec},
    transport::Body,
};
use topos_core::api::tce::v1::watch_certificates_request::{OpenStream, PauseStream};
use topos_core::api::tce::v1::WatchCertificatesRequest;
use uuid::Uuid;

use crate::InternalRuntimeCommand;
macro_rules! wait_for_command {
    ($node:expr, matches: $(|)? $( $pattern:pat_param )|+ $( if $guard: expr )? $(,)?) => {
        let assertion = async {
            while let Some(command) = $node.recv().await {
                if matches!(command, $( $pattern )|+ $( if $guard )?) {
                    break;
                }
            }
        };

        if let Err(_) = tokio::time::timeout(std::time::Duration::from_millis(100), assertion).await
        {
            panic!("Timeout waiting for command");
        }
    };
}
// Utility to encode our proto into GRPC stream format.
fn encode<T: Message>(proto: &T) -> Result<Bytes, Box<dyn std::error::Error>> {
    let mut buf = BytesMut::new();
    // See below comment on spec.
    use std::mem::size_of;
    const PREFIX_BYTES: usize = size_of::<u8>() + size_of::<u32>();
    for _ in 0..PREFIX_BYTES {
        // Advance our buffer first.
        // We will backfill it once we know the size of the message.
        buf.put_u8(0);
    }
    proto.encode(&mut buf)?;
    let len = buf.len() - PREFIX_BYTES;
    {
        let mut buf = &mut buf[0..PREFIX_BYTES];
        // See: https://github.com/grpc/grpc/blob/master/doc/PROTOCOL-HTTP2.md#:~:text=Compressed-Flag
        // for more details on spec.
        // Compressed-Flag -> 0 / 1 # encoded as 1 byte unsigned integer.
        buf.put_u8(0);
        // Message-Length -> {length of Message} # encoded as 4 byte unsigned integer (big endian).
        buf.put_u32(len as u32);
        // Message -> *{binary octet}.
    }

    Ok(buf.freeze())
}
mod utils {
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
        stream::{Stream, StreamCommand},
        InternalRuntimeCommand,
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
        pub fn outbound_stream_channel_size(mut self, value: usize) -> Self {
            self.outbound_stream_channel_size = value;

            self
        }

        pub fn runtime_channel_size(mut self, value: usize) -> Self {
            self.runtime_channel_size = value;

            self
        }

        pub fn stream_channel_size(mut self, value: usize) -> Self {
            self.stream_channel_size = value;

            self
        }

        pub fn stream_id(mut self, value: Uuid) -> Self {
            self.stream_id = value;

            self
        }

        pub fn build(self) -> (Sender, Stream, StreamContext) {
            let (tx, stream) = create_stream();
            let (sender, mut stream_receiver) = mpsc::channel(self.outbound_stream_channel_size);
            let (command_sender, command_receiver) = mpsc::channel(self.stream_channel_size);
            let (internal_runtime_command_sender, mut runtime_receiver) =
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
        pub(crate) command_sender: mpsc::Sender<StreamCommand>,
        pub(crate) runtime_receiver: mpsc::Receiver<InternalRuntimeCommand>,
        pub(crate) stream_id: Uuid,
    }
}

#[test(tokio::test)]
pub async fn sending_pause_stream_before_open() -> Result<(), Box<dyn std::error::Error>> {
    let (mut tx, stream, mut context) = utils::StreamBuilder::default().build();

    let msg: WatchCertificatesRequest = PauseStream {}.into();
    {
        _ = tx.send_data(encode(&msg)?).await;
        let _ = tx;
    }

    spawn(stream.run());

    wait_for_command!(
        context.stream_receiver,
        matches: Err(status) if status.message() == "No openstream provided"
    );

    let expected_stream_id = context.stream_id;

    wait_for_command!(
        context.runtime_receiver,
        matches: InternalRuntimeCommand::StreamTimeout { stream_id } if stream_id == expected_stream_id
    );

    Ok(())
}

#[test(tokio::test)]
pub async fn sending_no_message() -> Result<(), Box<dyn std::error::Error>> {
    let (_, stream, mut context) = utils::StreamBuilder::default().build();

    spawn(stream.run());

    wait_for_command!(
        context.stream_receiver,
        matches: Err(status) if status.message() == "No openstream provided"
    );

    let expected_stream_id = context.stream_id;

    wait_for_command!(
        context.runtime_receiver,
        matches: InternalRuntimeCommand::StreamTimeout { stream_id } if stream_id == expected_stream_id
    );

    Ok(())
}

#[test(tokio::test)]
pub async fn sending_open_stream_message() -> Result<(), Box<dyn std::error::Error>> {
    let (mut tx, stream, mut context) = utils::StreamBuilder::default().build();

    spawn(async move { stream.run().await });

    let msg: WatchCertificatesRequest = OpenStream {
        subnet_id: "subnet_id".to_string(),
    }
    .into();
    {
        _ = tx.send_data(encode(&msg)?).await;
        let _ = tx;
    }

    let expected_stream_id = context.stream_id;

    wait_for_command!(
        context.runtime_receiver,
        matches: InternalRuntimeCommand::Register { stream_id, ref subnet_id, .. } if stream_id == expected_stream_id && subnet_id == "subnet_id"
    );

    Ok(())
}
