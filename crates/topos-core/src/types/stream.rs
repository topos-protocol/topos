use std::fmt;

use serde::{Deserialize, Serialize};
use thiserror::Error;

use topos_uci::SubnetId;

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
pub struct CertificateSourceStreamPosition {
    // Source subnet id
    pub subnet_id: SubnetId,
    // Source certificate position
    pub position: Position,
}

impl fmt::Display for CertificateSourceStreamPosition {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}:{}", self.subnet_id, self.position)
    }
}

impl CertificateSourceStreamPosition {
    pub fn new(subnet_id: SubnetId, position: Position) -> Self {
        Self {
            subnet_id,
            position,
        }
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
