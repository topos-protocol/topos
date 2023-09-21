use std::{fmt, ops::Deref};

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
    pub fn new<P: Into<Position>>(subnet_id: SubnetId, position: P) -> Self {
        Self {
            subnet_id,
            position: position.into(),
        }
    }
}

/// Certificate index in the history of the source subnet
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq, Copy)]
pub struct Position(u64);

impl TryInto<usize> for Position {
    type Error = PositionError;

    fn try_into(self) -> Result<usize, Self::Error> {
        self.0
            .try_into()
            .map_err(|_| PositionError::InvalidPosition)
    }
}

impl TryFrom<usize> for Position {
    type Error = PositionError;

    fn try_from(value: usize) -> Result<Self, Self::Error> {
        Ok(Self(
            value
                .try_into()
                .map_err(|_| PositionError::InvalidPosition)?,
        ))
    }
}

impl From<u64> for Position {
    fn from(value: u64) -> Self {
        Self(value)
    }
}

impl Deref for Position {
    type Target = u64;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl PartialEq<Position> for u64 {
    fn eq(&self, other: &Position) -> bool {
        *self == other.0
    }
}

impl PartialOrd for Position {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        self.0.partial_cmp(&other.0)
    }
}

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

    #[error("")]
    InvalidPosition,
}
