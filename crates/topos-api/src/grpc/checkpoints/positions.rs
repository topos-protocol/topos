use crate::grpc::checkpoints::StreamPositionError;
use crate::grpc::shared::v1 as shared_v1;
use topos_uci::{CertificateId, SubnetId};

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct TargetStreamPosition {
    pub target_subnet_id: SubnetId,
    pub source_subnet_id: SubnetId,
    pub position: u64,
    pub certificate_id: Option<CertificateId>,
}

impl TryFrom<shared_v1::positions::TargetStreamPosition> for TargetStreamPosition {
    type Error = StreamPositionError;

    fn try_from(value: shared_v1::positions::TargetStreamPosition) -> Result<Self, Self::Error> {
        Ok(Self {
            target_subnet_id: value
                .target_subnet_id
                .map(TryInto::try_into)
                .ok_or(StreamPositionError::MissingTargetSubnetId)??,
            source_subnet_id: value
                .source_subnet_id
                .map(TryInto::try_into)
                .ok_or(StreamPositionError::MissingSourceSubnetId)??,
            position: value.position,
            certificate_id: value
                .certificate_id
                .map(TryInto::try_into)
                .map_or(Ok(None), |v| {
                    v.map(Some)
                        .map_err(|_| StreamPositionError::InvalidCertificateIdFormat)
                })?,
        })
    }
}

impl From<TargetStreamPosition> for shared_v1::positions::TargetStreamPosition {
    fn from(value: TargetStreamPosition) -> Self {
        Self {
            source_subnet_id: Some(value.source_subnet_id.into()),
            target_subnet_id: Some(value.target_subnet_id.into()),
            position: value.position,
            certificate_id: value.certificate_id.map(Into::into),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct SourceStreamPosition {
    pub source_subnet_id: SubnetId,
    pub position: u64,
    pub certificate_id: Option<CertificateId>,
}

impl TryFrom<shared_v1::positions::SourceStreamPosition> for SourceStreamPosition {
    type Error = StreamPositionError;

    fn try_from(value: shared_v1::positions::SourceStreamPosition) -> Result<Self, Self::Error> {
        Ok(Self {
            source_subnet_id: value
                .source_subnet_id
                .map(TryInto::try_into)
                .ok_or(StreamPositionError::MissingSourceSubnetId)??,
            position: value.position,
            certificate_id: value
                .certificate_id
                .map(TryInto::try_into)
                .map_or(Ok(None), |v| {
                    v.map(Some)
                        .map_err(|_| StreamPositionError::InvalidCertificateIdFormat)
                })?,
        })
    }
}

impl From<SourceStreamPosition> for shared_v1::positions::SourceStreamPosition {
    fn from(value: SourceStreamPosition) -> Self {
        Self {
            source_subnet_id: Some(value.source_subnet_id.into()),
            position: value.position,
            certificate_id: value.certificate_id.map(Into::into),
        }
    }
}
