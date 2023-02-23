use crate::shared::v1 as shared_v1;
use topos_uci::SubnetId;

mod errors;
mod positions;

pub use errors::*;
pub use positions::*;

#[derive(Debug, Default)]
pub struct TargetCheckpoint {
    pub target_subnet_ids: Vec<SubnetId>,
    pub positions: Vec<TargetStreamPosition>,
}

impl TryFrom<shared_v1::checkpoints::TargetCheckpoint> for TargetCheckpoint {
    type Error = TargetCheckpointError;

    fn try_from(value: shared_v1::checkpoints::TargetCheckpoint) -> Result<Self, Self::Error> {
        Ok(TargetCheckpoint {
            target_subnet_ids: value
                .target_subnet_ids
                .into_iter()
                .map(TryInto::try_into)
                .collect::<Result<Vec<SubnetId>, _>>()
                .map_err(|_| TargetCheckpointError::InvalidSubnetFormat)?,
            positions: value
                .positions
                .into_iter()
                .map(TryInto::try_into)
                .collect::<Result<Vec<TargetStreamPosition>, _>>()
                .map_err(|_| TargetCheckpointError::InvalidTargetStreamPosition)?,
        })
    }
}

impl From<TargetCheckpoint> for shared_v1::checkpoints::TargetCheckpoint {
    fn from(value: TargetCheckpoint) -> Self {
        Self {
            target_subnet_ids: value
                .target_subnet_ids
                .into_iter()
                .map(Into::into)
                .collect(),
            positions: value.positions.into_iter().map(Into::into).collect(),
        }
    }
}
