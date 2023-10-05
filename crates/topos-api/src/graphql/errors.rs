#[derive(Debug, thiserror::Error)]
pub enum GraphQLServerError {
    #[error("The provided data layer is invalid")]
    ParseDataConnector,

    #[error("The provided subnet_id is not a proper HEX value")]
    ParseSubnetId,

    #[error("The provided certificate_id is not a proper HEX value")]
    ParseCertificateId,

    #[error("Internal Server Error")]
    StorageError,

    #[error("Certificate not found")]
    CertificateNotFound,

    #[error("Unable to create transient stream: {0}")]
    TransientStream(String),

    #[error("Internal API error: {0}")]
    InternalError(&'static str),
}
