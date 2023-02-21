use topos_core::{
    api::shared::v1::{
        checkpoints::TargetCheckpoint as GrpcTargetCheckpoint,
        positions::TargetStreamPosition as GrpcTargetStreamPosition,
    },
    uci::{CertificateId, SubnetId},
};

#[derive(Debug, Default)]
pub struct TargetCheckpoint {
    pub target_subnet_ids: Vec<SubnetId>,
    pub positions: Vec<TargetStreamPosition>,
}

#[derive(Debug)]
pub struct SourceCheckpoint {
    pub source_subnet_ids: Vec<SubnetId>,
    pub positions: Vec<SourceStreamPosition>,
}

#[derive(Debug)]
pub struct SourceStreamPosition {
    pub source_subnet_id: SubnetId,
    pub position: u64,
    pub certificate_id: Option<CertificateId>,
}

#[derive(Debug, Clone)]
pub struct TargetStreamPosition {
    pub target_subnet_id: SubnetId,
    pub source_subnet_id: SubnetId,
    pub position: u64,
    pub certificate_id: Option<CertificateId>,
}

pub enum TargetCheckpointError {
    ParseError,
}

pub enum TargetStreamPositionError {
    ParseError,
}

impl TryFrom<Option<GrpcTargetCheckpoint>> for TargetCheckpoint {
    type Error = TargetCheckpointError;

    fn try_from(value: Option<GrpcTargetCheckpoint>) -> Result<Self, Self::Error> {
        match value {
            Some(target) => Ok(TargetCheckpoint {
                target_subnet_ids: target
                    .target_subnet_ids
                    .into_iter()
                    .map(TryInto::try_into)
                    .collect::<Result<Vec<SubnetId>, _>>()
                    .map_err(|_| TargetCheckpointError::ParseError)?,
                positions: target
                    .positions
                    .into_iter()
                    .map(TryInto::try_into)
                    .collect::<Result<Vec<TargetStreamPosition>, _>>()
                    .map_err(|_| TargetCheckpointError::ParseError)?,
            }),
            None => Err(TargetCheckpointError::ParseError),
        }
    }
}

impl TryFrom<GrpcTargetStreamPosition> for TargetStreamPosition {
    type Error = TargetStreamPositionError;

    fn try_from(value: GrpcTargetStreamPosition) -> Result<Self, Self::Error> {
        Ok(Self {
            target_subnet_id: value
                .target_subnet_id
                .ok_or(TargetStreamPositionError::ParseError)?
                .try_into()
                .map_err(|_| TargetStreamPositionError::ParseError)?,
            source_subnet_id: value
                .source_subnet_id
                .ok_or(TargetStreamPositionError::ParseError)?
                .try_into()
                .map_err(|_| TargetStreamPositionError::ParseError)?,
            position: value.position,
            certificate_id: value
                .certificate_id
                .map(TryInto::try_into)
                .map_or(Ok(None), |v| {
                    v.map(Some)
                        .map_err(|_| TargetStreamPositionError::ParseError)
                })?,
        })
    }
}
