use std::io;

use crate::constant::{SYNCHRONIZER_PROTOCOL, TRANSMISSION_PROTOCOL};

use super::protocol::TransmissionProtocol;
use futures::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};
use libp2p::{
    core::upgrade::{read_length_prefixed, write_length_prefixed},
    request_response::Codec,
    StreamProtocol,
};

#[derive(Clone)]
pub struct TransmissionCodec();

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TransmissionRequest {
    Transmission(Vec<u8>),
    Synchronizer(Vec<u8>),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TransmissionResponse {
    Transmission(Vec<u8>),
    Synchronizer(Vec<u8>),
}
const MAX_PACKET_SIZE: u64 = 16 * 1024 * 1024;
#[async_trait::async_trait]
impl Codec for TransmissionCodec {
    type Protocol = StreamProtocol;
    type Request = TransmissionRequest;
    type Response = Result<TransmissionResponse, ()>;

    async fn read_request<T>(
        &mut self,
        protocol: &Self::Protocol,
        mut io: &mut T,
    ) -> std::io::Result<Self::Request>
    where
        T: AsyncRead + Unpin + Send,
    {
        // Read the length.
        let length = unsigned_varint::aio::read_usize(&mut io)
            .await
            .map_err(|err| io::Error::new(io::ErrorKind::InvalidInput, err))?;
        if length > usize::try_from(MAX_PACKET_SIZE).unwrap_or(usize::MAX) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!(
                    "Request size exceeds limit: {} > {}",
                    length, MAX_PACKET_SIZE
                ),
            ));
        }

        // Read the payload.
        let mut buffer = vec![0; length];
        io.read_exact(&mut buffer).await?;

        match protocol.as_ref() {
            SYNCHRONIZER_PROTOCOL => Ok(TransmissionRequest::Synchronizer(buffer)),
            _ => Ok(TransmissionRequest::Transmission(buffer)),
        }
    }

    async fn read_response<T>(
        &mut self,
        protocol: &Self::Protocol,
        mut io: &mut T,
    ) -> std::io::Result<Self::Response>
    where
        T: AsyncRead + Unpin + Send,
    {
        // Read the length of the payload.
        let length = match unsigned_varint::aio::read_usize(&mut io).await {
            Ok(l) => l,
            Err(unsigned_varint::io::ReadError::Io(err))
                if matches!(err.kind(), io::ErrorKind::UnexpectedEof) =>
            {
                return Ok(Err(()))
            }
            Err(err) => return Err(io::Error::new(io::ErrorKind::InvalidInput, err)),
        };

        if length > usize::try_from(MAX_PACKET_SIZE).unwrap_or(usize::MAX) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!(
                    "Response size exceeds limit: {} > {}",
                    length, MAX_PACKET_SIZE
                ),
            ));
        }

        // Read the payload itself.
        let mut buffer = vec![0; length];
        io.read_exact(&mut buffer).await?;

        let res = match protocol.as_ref() {
            SYNCHRONIZER_PROTOCOL => Ok(TransmissionResponse::Synchronizer(buffer)),
            _ => Ok(TransmissionResponse::Transmission(buffer)),
        };

        Ok(res)
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
        let req = match req {
            TransmissionRequest::Transmission(req) => req,
            TransmissionRequest::Synchronizer(req) => req,
        };

        {
            // Write the length first
            let mut buffer = unsigned_varint::encode::usize_buffer();
            io.write_all(unsigned_varint::encode::usize(req.len(), &mut buffer))
                .await?;
        }

        // Write the payload.
        io.write_all(&req).await?;

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
        if let Ok(res) = res {
            let res = match res {
                TransmissionResponse::Synchronizer(res) => res,
                TransmissionResponse::Transmission(res) => res,
            };

            {
                // Write the length
                let mut buffer = unsigned_varint::encode::usize_buffer();
                io.write_all(unsigned_varint::encode::usize(res.len(), &mut buffer))
                    .await?;
            }

            // Write the payload.
            io.write_all(&res).await?;
        }

        io.close().await?;

        Ok(())
    }
}
