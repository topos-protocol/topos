use crate::shared::v1_conversions_subnet::Error;

#[derive(Debug, thiserror::Error)]
pub enum TargetCheckpointError {
    #[error("Subnet format is invalid")]
    InvalidSubnetFormat,
    #[error("Invalid target stream position")]
    InvalidTargetStreamPosition,
    #[error("Checkpoint parse error")]
    ParseError,
}

#[derive(Debug, thiserror::Error)]
pub enum StreamPositionError {
    #[error("The target_subnet_id field is missing")]
    MissingTargetSubnetId,
    #[error("The source_subnet_id field is missing")]
    MissingSourceSubnetId,
    #[error("Unable to parse SubnetId: {0}")]
    InvalidSubnetFormat(#[from] Error),
    #[error("Unable to parse CertificateId")]
    InvalidCertificateIdFormat,
}
