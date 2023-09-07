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
}
