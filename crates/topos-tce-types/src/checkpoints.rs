use topos_core::{
    api::shared::v1::{
        checkpoints::TargetCheckpoint as GrpcTargetCheckpoint,
        positions::TargetStreamPosition as GrpcTargetStreamPosition,
    },
    uci::{CertificateId, SubnetId},
};

#[derive(Default)]
pub struct TargetCheckpoint {
    pub target_subnet_ids: Vec<SubnetId>,
    pub positions: Vec<TargetStreamPosition>,
}
pub struct SourceCheckpoint {}

#[derive(Debug)]
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
                positions: vec![],
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
                .into(),
            source_subnet_id: value
                .source_subnet_id
                .ok_or(TargetStreamPositionError::ParseError)?
                .into(),
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
