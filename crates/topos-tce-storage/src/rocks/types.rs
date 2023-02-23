use serde::{Deserialize, Serialize};
use topos_core::uci::{Certificate, CertificateId};

use crate::{Position, SubnetId};

use super::db_column::DBColumn;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct TargetStreamPosition(
    pub(crate) SubnetId,
    pub(crate) SubnetId,
    pub(crate) Position,
);

#[derive(Debug, Serialize, Deserialize)]
pub(crate) struct TargetStreamPrefix(pub(crate) SubnetId, pub(crate) SubnetId);

#[derive(Debug, Serialize, Deserialize, Clone)]
pub(crate) struct SourceStreamPosition(pub(crate) SubnetId, pub(crate) Position);

pub(crate) type PendingCertificatesColumn = DBColumn<u64, Certificate>;
pub(crate) type CertificatesColumn = DBColumn<CertificateId, Certificate>;
pub(crate) type SourceStreamsColumn = DBColumn<SourceStreamPosition, CertificateId>;
pub(crate) type TargetStreamsColumn = DBColumn<TargetStreamPosition, CertificateId>;
pub(crate) type TargetSourceListColumn = DBColumn<TargetStreamPrefix, u64>;
