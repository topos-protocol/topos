use serde::{Deserialize, Serialize};

use crate::SubnetId;

#[derive(Debug, Serialize, Deserialize)]
pub(crate) struct TargetSourceListKey(
    // Target subnet id
    pub(crate) SubnetId,
    // Source subnet id
    pub(crate) SubnetId,
);
