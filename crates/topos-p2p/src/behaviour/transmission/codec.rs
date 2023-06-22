use std::io;

use crate::constant::TRANSMISSION_PROTOCOL;

use super::protocol::TransmissionProtocol;
use futures::{AsyncRead, AsyncWrite, AsyncWriteExt};
use libp2p::{
    core::upgrade::{read_length_prefixed, write_length_prefixed},
    request_response::Codec,
    StreamProtocol,
};

#[derive(Clone)]
pub(crate) struct TransmissionCodec();

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TransmissionRequest(pub(crate) Vec<u8>);

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TransmissionResponse(pub(crate) Vec<u8>);

#[async_trait::async_trait]
impl Codec for TransmissionCodec {
    type Protocol = StreamProtocol;
    type Request = TransmissionRequest;
    type Response = TransmissionResponse;

    async fn read_request<T>(
        &mut self,
        _: &Self::Protocol,
        io: &mut T,
    ) -> std::io::Result<Self::Request>
    where
        T: AsyncRead + Unpin + Send,
    {
        let vec = read_length_prefixed(io, 1_000_000_000).await?;
        Ok(TransmissionRequest(vec))
    }

    async fn read_response<T>(
        &mut self,
        _: &Self::Protocol,
        io: &mut T,
    ) -> std::io::Result<Self::Response>
    where
        T: AsyncRead + Unpin + Send,
    {
        let vec = read_length_prefixed(io, 1_000_000).await?;

        if vec.is_empty() {
            return Err(io::ErrorKind::UnexpectedEof.into());
        }

        Ok(TransmissionResponse(vec))
    }

    async fn write_request<T>(
        &mut self,
        _: &Self::Protocol,
        io: &mut T,
        req: Self::Request,
    ) -> std::io::Result<()>
    where
        T: AsyncWrite + Unpin + Send,
    {
        write_length_prefixed(io, req.0).await?;
        io.close().await?;

        Ok(())
    }

    async fn write_response<T>(
        &mut self,
        _: &Self::Protocol,
        io: &mut T,
        res: Self::Response,
    ) -> std::io::Result<()>
    where
        T: AsyncWrite + Unpin + Send,
    {
        write_length_prefixed(io, res.0).await?;
        io.close().await?;

        Ok(())
    }
}
