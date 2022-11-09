use serde::{Deserialize, Serialize};
use topos_core::uci::{Certificate, CertificateId};

use crate::{Height, SubnetId};

use super::db_column::DBColumn;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub(crate) struct TargetStreamRef(pub(crate) SubnetId, pub(crate) SubnetId, pub(crate) Height);

#[derive(Debug, Serialize, Deserialize)]
pub(crate) struct TargetStreamPrefix(pub(crate) SubnetId, pub(crate) SubnetId);

#[derive(Debug, Serialize, Deserialize, Clone)]
pub(crate) struct SourceStreamRef(pub(crate) SubnetId, pub(crate) Height);

pub(crate) type PendingCertificatesColumn = DBColumn<u64, Certificate>;
pub(crate) type CertificatesColumn = DBColumn<CertificateId, Certificate>;
pub(crate) type SourceSubnetStreamsColumn = DBColumn<SourceStreamRef, CertificateId>;
pub(crate) type TargetSubnetStreamsColumn = DBColumn<TargetStreamRef, CertificateId>;
