#[derive(Clone, PartialEq, ::prost::Message)]
pub struct WatchUpdatesRequest {
}
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct WatchUpdatesResponse {
    #[prost(message, optional, tag="1")]
    pub now: ::core::option::Option<::prost_types::Timestamp>,
    #[prost(message, optional, tag="2")]
    pub initial_state: ::core::option::Option<InitialState>,
    #[prost(message, repeated, tag="3")]
    pub network_update: ::prost::alloc::vec::Vec<NetworkUpdate>,
    #[prost(message, repeated, tag="4")]
    pub runtime_update: ::prost::alloc::vec::Vec<RuntimeUpdate>,
    #[prost(message, repeated, tag="5")]
    pub double_echo_update: ::prost::alloc::vec::Vec<DoubleEchoUpdate>,
}
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct NetworkUpdate {
    #[prost(oneof="network_update::Update", tags="1, 2")]
    pub update: ::core::option::Option<network_update::Update>,
}
/// Nested message and enum types in `NetworkUpdate`.
pub mod network_update {
    #[derive(Clone, PartialEq, ::prost::Oneof)]
    pub enum Update {
        #[prost(message, tag="1")]
        ConnectedPeer(super::ConnectedPeer),
        #[prost(message, tag="2")]
        DisconnectedPeer(super::DisconnectedPeer),
    }
}
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct DoubleEchoUpdate {
    #[prost(oneof="double_echo_update::Update", tags="1")]
    pub update: ::core::option::Option<double_echo_update::Update>,
}
/// Nested message and enum types in `DoubleEchoUpdate`.
pub mod double_echo_update {
    #[derive(Clone, PartialEq, ::prost::Oneof)]
    pub enum Update {
        #[prost(message, tag="1")]
        NewSample(super::NewSample),
    }
}
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct ConnectedPeer {
    #[prost(string, tag="1")]
    pub peer_id: ::prost::alloc::string::String,
    #[prost(string, tag="2")]
    pub addresses: ::prost::alloc::string::String,
    #[prost(string, tag="3")]
    pub direction: ::prost::alloc::string::String,
}
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct DisconnectedPeer {
    #[prost(string, tag="1")]
    pub peer_id: ::prost::alloc::string::String,
}
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct RuntimeUpdate {
    #[prost(oneof="runtime_update::Update", tags="1")]
    pub update: ::core::option::Option<runtime_update::Update>,
}
/// Nested message and enum types in `RuntimeUpdate`.
pub mod runtime_update {
    #[derive(Clone, PartialEq, ::prost::Oneof)]
    pub enum Update {
        #[prost(message, tag="1")]
        NewCertificate(super::NewCertificate),
    }
}
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct NewCertificate {
    #[prost(string, tag="1")]
    pub cert_id: ::prost::alloc::string::String,
    #[prost(string, tag="2")]
    pub subnet_id: ::prost::alloc::string::String,
}
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct PendingCertificate {
    #[prost(string, tag="1")]
    pub cert_id: ::prost::alloc::string::String,
    #[prost(string, tag="2")]
    pub subnet_id: ::prost::alloc::string::String,
}
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct ValidatedCertificate {
    #[prost(string, tag="1")]
    pub cert_id: ::prost::alloc::string::String,
    #[prost(string, tag="2")]
    pub subnet_id: ::prost::alloc::string::String,
}
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct Stream {
    #[prost(string, tag="1")]
    pub stream_uuid: ::prost::alloc::string::String,
    #[prost(string, repeated, tag="2")]
    pub subnet_ids: ::prost::alloc::vec::Vec<::prost::alloc::string::String>,
}
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct Sample {
    #[prost(string, repeated, tag="1")]
    pub echo_subscribers: ::prost::alloc::vec::Vec<::prost::alloc::string::String>,
    #[prost(string, repeated, tag="2")]
    pub ready_subscribers: ::prost::alloc::vec::Vec<::prost::alloc::string::String>,
    #[prost(string, repeated, tag="3")]
    pub echo_subsriptions: ::prost::alloc::vec::Vec<::prost::alloc::string::String>,
    #[prost(string, repeated, tag="4")]
    pub ready_subsriptions: ::prost::alloc::vec::Vec<::prost::alloc::string::String>,
    #[prost(string, repeated, tag="5")]
    pub delivery_subsriptions: ::prost::alloc::vec::Vec<::prost::alloc::string::String>,
}
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct InitialState {
    #[prost(message, repeated, tag="1")]
    pub peers: ::prost::alloc::vec::Vec<ConnectedPeer>,
    #[prost(message, repeated, tag="2")]
    pub pending_certificate: ::prost::alloc::vec::Vec<PendingCertificate>,
    #[prost(message, repeated, tag="3")]
    pub validated_certificate: ::prost::alloc::vec::Vec<ValidatedCertificate>,
    #[prost(message, optional, tag="4")]
    pub current_sample: ::core::option::Option<Sample>,
    #[prost(message, repeated, tag="5")]
    pub streams: ::prost::alloc::vec::Vec<Stream>,
}
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct NewSample {
    #[prost(message, optional, tag="1")]
    pub sample: ::core::option::Option<Sample>,
}
/// Generated client implementations.
pub mod instrument_client {
    #![allow(unused_variables, dead_code, missing_docs, clippy::let_unit_value)]
    use tonic::codegen::*;
    use tonic::codegen::http::Uri;
    #[derive(Debug, Clone)]
    pub struct InstrumentClient<T> {
        inner: tonic::client::Grpc<T>,
    }
    impl InstrumentClient<tonic::transport::Channel> {
        /// Attempt to create a new client by connecting to a given endpoint.
        pub async fn connect<D>(dst: D) -> Result<Self, tonic::transport::Error>
        where
            D: std::convert::TryInto<tonic::transport::Endpoint>,
            D::Error: Into<StdError>,
        {
            let conn = tonic::transport::Endpoint::new(dst)?.connect().await?;
            Ok(Self::new(conn))
        }
    }
    impl<T> InstrumentClient<T>
    where
        T: tonic::client::GrpcService<tonic::body::BoxBody>,
        T::Error: Into<StdError>,
        T::ResponseBody: Body<Data = Bytes> + Send + 'static,
        <T::ResponseBody as Body>::Error: Into<StdError> + Send,
    {
        pub fn new(inner: T) -> Self {
            let inner = tonic::client::Grpc::new(inner);
            Self { inner }
        }
        pub fn with_origin(inner: T, origin: Uri) -> Self {
            let inner = tonic::client::Grpc::with_origin(inner, origin);
            Self { inner }
        }
        pub fn with_interceptor<F>(
            inner: T,
            interceptor: F,
        ) -> InstrumentClient<InterceptedService<T, F>>
        where
            F: tonic::service::Interceptor,
            T::ResponseBody: Default,
            T: tonic::codegen::Service<
                http::Request<tonic::body::BoxBody>,
                Response = http::Response<
                    <T as tonic::client::GrpcService<tonic::body::BoxBody>>::ResponseBody,
                >,
            >,
            <T as tonic::codegen::Service<
                http::Request<tonic::body::BoxBody>,
            >>::Error: Into<StdError> + Send + Sync,
        {
            InstrumentClient::new(InterceptedService::new(inner, interceptor))
        }
        /// Compress requests with the given encoding.
        ///
        /// This requires the server to support it otherwise it might respond with an
        /// error.
        #[must_use]
        pub fn send_compressed(mut self, encoding: CompressionEncoding) -> Self {
            self.inner = self.inner.send_compressed(encoding);
            self
        }
        /// Enable decompressing responses.
        #[must_use]
        pub fn accept_compressed(mut self, encoding: CompressionEncoding) -> Self {
            self.inner = self.inner.accept_compressed(encoding);
            self
        }
        pub async fn watch_updates(
            &mut self,
            request: impl tonic::IntoRequest<super::WatchUpdatesRequest>,
        ) -> Result<
            tonic::Response<tonic::codec::Streaming<super::WatchUpdatesResponse>>,
            tonic::Status,
        > {
            self.inner
                .ready()
                .await
                .map_err(|e| {
                    tonic::Status::new(
                        tonic::Code::Unknown,
                        format!("Service was not ready: {}", e.into()),
                    )
                })?;
            let codec = tonic::codec::ProstCodec::default();
            let path = http::uri::PathAndQuery::from_static(
                "/console.instrument.Instrument/WatchUpdates",
            );
            self.inner.server_streaming(request.into_request(), path, codec).await
        }
    }
}
/// Generated server implementations.
pub mod instrument_server {
    #![allow(unused_variables, dead_code, missing_docs, clippy::let_unit_value)]
    use tonic::codegen::*;
    ///Generated trait containing gRPC methods that should be implemented for use with InstrumentServer.
    #[async_trait]
    pub trait Instrument: Send + Sync + 'static {
        ///Server streaming response type for the WatchUpdates method.
        type WatchUpdatesStream: futures_core::Stream<
                Item = Result<super::WatchUpdatesResponse, tonic::Status>,
            >
            + Send
            + 'static;
        async fn watch_updates(
            &self,
            request: tonic::Request<super::WatchUpdatesRequest>,
        ) -> Result<tonic::Response<Self::WatchUpdatesStream>, tonic::Status>;
    }
    #[derive(Debug)]
    pub struct InstrumentServer<T: Instrument> {
        inner: _Inner<T>,
        accept_compression_encodings: EnabledCompressionEncodings,
        send_compression_encodings: EnabledCompressionEncodings,
    }
    struct _Inner<T>(Arc<T>);
    impl<T: Instrument> InstrumentServer<T> {
        pub fn new(inner: T) -> Self {
            Self::from_arc(Arc::new(inner))
        }
        pub fn from_arc(inner: Arc<T>) -> Self {
            let inner = _Inner(inner);
            Self {
                inner,
                accept_compression_encodings: Default::default(),
                send_compression_encodings: Default::default(),
            }
        }
        pub fn with_interceptor<F>(
            inner: T,
            interceptor: F,
        ) -> InterceptedService<Self, F>
        where
            F: tonic::service::Interceptor,
        {
            InterceptedService::new(Self::new(inner), interceptor)
        }
        /// Enable decompressing requests with the given encoding.
        #[must_use]
        pub fn accept_compressed(mut self, encoding: CompressionEncoding) -> Self {
            self.accept_compression_encodings.enable(encoding);
            self
        }
        /// Compress responses with the given encoding, if the client supports it.
        #[must_use]
        pub fn send_compressed(mut self, encoding: CompressionEncoding) -> Self {
            self.send_compression_encodings.enable(encoding);
            self
        }
    }
    impl<T, B> tonic::codegen::Service<http::Request<B>> for InstrumentServer<T>
    where
        T: Instrument,
        B: Body + Send + 'static,
        B::Error: Into<StdError> + Send + 'static,
    {
        type Response = http::Response<tonic::body::BoxBody>;
        type Error = std::convert::Infallible;
        type Future = BoxFuture<Self::Response, Self::Error>;
        fn poll_ready(
            &mut self,
            _cx: &mut Context<'_>,
        ) -> Poll<Result<(), Self::Error>> {
            Poll::Ready(Ok(()))
        }
        fn call(&mut self, req: http::Request<B>) -> Self::Future {
            let inner = self.inner.clone();
            match req.uri().path() {
                "/console.instrument.Instrument/WatchUpdates" => {
                    #[allow(non_camel_case_types)]
                    struct WatchUpdatesSvc<T: Instrument>(pub Arc<T>);
                    impl<
                        T: Instrument,
                    > tonic::server::ServerStreamingService<super::WatchUpdatesRequest>
                    for WatchUpdatesSvc<T> {
                        type Response = super::WatchUpdatesResponse;
                        type ResponseStream = T::WatchUpdatesStream;
                        type Future = BoxFuture<
                            tonic::Response<Self::ResponseStream>,
                            tonic::Status,
                        >;
                        fn call(
                            &mut self,
                            request: tonic::Request<super::WatchUpdatesRequest>,
                        ) -> Self::Future {
                            let inner = self.0.clone();
                            let fut = async move {
                                (*inner).watch_updates(request).await
                            };
                            Box::pin(fut)
                        }
                    }
                    let accept_compression_encodings = self.accept_compression_encodings;
                    let send_compression_encodings = self.send_compression_encodings;
                    let inner = self.inner.clone();
                    let fut = async move {
                        let inner = inner.0;
                        let method = WatchUpdatesSvc(inner);
                        let codec = tonic::codec::ProstCodec::default();
                        let mut grpc = tonic::server::Grpc::new(codec)
                            .apply_compression_config(
                                accept_compression_encodings,
                                send_compression_encodings,
                            );
                        let res = grpc.server_streaming(method, req).await;
                        Ok(res)
                    };
                    Box::pin(fut)
                }
                _ => {
                    Box::pin(async move {
                        Ok(
                            http::Response::builder()
                                .status(200)
                                .header("grpc-status", "12")
                                .header("content-type", "application/grpc")
                                .body(empty_body())
                                .unwrap(),
                        )
                    })
                }
            }
        }
    }
    impl<T: Instrument> Clone for InstrumentServer<T> {
        fn clone(&self) -> Self {
            let inner = self.inner.clone();
            Self {
                inner,
                accept_compression_encodings: self.accept_compression_encodings,
                send_compression_encodings: self.send_compression_encodings,
            }
        }
    }
    impl<T: Instrument> Clone for _Inner<T> {
        fn clone(&self) -> Self {
            Self(self.0.clone())
        }
    }
    impl<T: std::fmt::Debug> std::fmt::Debug for _Inner<T> {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            write!(f, "{:?}", self.0)
        }
    }
    impl<T: Instrument> tonic::server::NamedService for InstrumentServer<T> {
        const NAME: &'static str = "console.instrument.Instrument";
    }
}
