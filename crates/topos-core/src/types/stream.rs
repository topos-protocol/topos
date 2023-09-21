use std::{fmt, ops::Deref};

use serde::{Deserialize, Serialize};
use thiserror::Error;

use topos_uci::SubnetId;

/// Represents the place of a certificate in the stream of the Source Subnet
///
/// The `Source` Subnet is the subnet that produced the certificate.
/// A certificate should and will have the same position in this stream
/// no matter which node or component delivered it. The position is an
/// aggregation of the precedence chain of a certificate, starting by the
/// genesis certificate represented by a certificate which have a prev_id
/// equal to the `INITIAL_CERTIFICATE_ID`
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

/// Represents the place of a certificate in the stream of a Target Subnet
///
/// A `Target` Subnet is a subnet that was defined as target by the certificate.
/// A certificate can have multiple target subnets, leading to multiple
/// CertificateTargetStreamPosition for the same certificate but never more than
/// one CertificateTargetStreamPosition per couple (target, source).
///
/// The position of a certificate in a target stream will be the same accross
/// the entire network.
#[derive(Debug, Deserialize, Serialize, Clone, Copy)]
pub struct CertificateTargetStreamPosition {
    pub target_subnet_id: SubnetId,
    pub source_subnet_id: SubnetId,
    pub position: Position,
}

impl CertificateTargetStreamPosition {
    pub fn new<P: Into<Position>>(
        target_subnet_id: SubnetId,
        source_subnet_id: SubnetId,
        position: P,
    ) -> Self {
        Self {
            target_subnet_id,
            source_subnet_id,
            position: position.into(),
        }
    }
}

/// Certificate index in a stream of both source or target subnet
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq, Copy)]
pub struct Position(u64);

impl TryFrom<Position> for usize {
    type Error = PositionError;

    fn try_from(position: Position) -> Result<usize, Self::Error> {
        position
            .0
            .try_into()
            .map_err(|_| PositionError::InvalidPosition)
    }
}

impl TryFrom<usize> for Position {
    type Error = PositionError;

    fn try_from(value: usize) -> Result<Self, Self::Error> {
        Ok(Self(
            u64::try_from(value).map_err(|_| PositionError::InvalidPosition)?,
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
