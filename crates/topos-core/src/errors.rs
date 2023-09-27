use topos_api::grpc::checkpoints::StreamPositionError;

#[derive(Debug, thiserror::Error)]
pub enum GrpcParsingError {
    #[error("Malformed gRPC object: {0}")]
    GrpcMalformedType(&'static str),
    #[error(transparent)]
    PositionParsing(#[from] StreamPositionError),
}
