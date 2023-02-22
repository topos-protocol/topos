use crate::shared::v1_conversions_subnet::Error;

pub enum TargetCheckpointError {
    InvalidSubnetFormat,
    InvalidTargetStreamPosition,
    ParseError,
}

#[derive(Debug, thiserror::Error)]
pub enum TargetStreamPositionError {
    #[error("The target_subnet_id field is missing")]
    MissingTargetSubnetId,
    #[error("The source_subnet_id field is missing")]
    MissingSourceSubnetId,
    #[error("Unable to parse SubnetId: {0}")]
    InvalidSubnetFormat(#[from] Error),
    #[error("Unable to parse CertificateId")]
    InvalidCertificateIdFormat,
}
