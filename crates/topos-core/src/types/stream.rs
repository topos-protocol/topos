use std::fmt;

use serde::{Deserialize, Serialize};
use thiserror::Error;

use topos_uci::SubnetId;

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
pub struct SourceStreamPositionKey(
    // Source subnet id
    pub SubnetId,
    // Source certificate position
    pub Position,
);

impl fmt::Display for SourceStreamPositionKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}:{}", self.0, self.1)
    }
}

/// Certificate index in the history of the source subnet
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq, Copy)]
pub struct Position(pub u64);

impl fmt::Display for Position {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl Position {
    pub const ZERO: Self = Self(0);

    pub fn increment(self) -> Result<Self, PositionError> {
        match self {
            Self::ZERO => Ok(Self(1)),
            Self(value) => value
                .checked_add(1)
                .ok_or(PositionError::MaximumPositionReached)
                .map(Self),
        }
    }
}

#[derive(Debug, Error)]
pub enum PositionError {
    #[error("Maximum position reached for subnet")]
    MaximumPositionReached,
    #[error("Invalid expected position")]
    InvalidExpectedPosition,
}
