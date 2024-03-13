use async_graphql::NewType;
use serde::{Deserialize, Serialize};
use std::str::FromStr;
use tracing::error;

use super::errors::GraphQLServerError;

#[derive(Clone, Debug, Serialize, Deserialize, NewType, PartialEq, Eq)]
pub struct SubnetId(pub(crate) String);

impl TryFrom<&SubnetId> for crate::uci::SubnetId {
    type Error = GraphQLServerError;

    fn try_from(value: &SubnetId) -> Result<Self, Self::Error> {
        Self::from_str(value.0.as_str()).map_err(|e| {
            error!("Failed to convert SubnetId from GraphQL input {e:?}");
            GraphQLServerError::ParseDataConnector
        })
    }
}

impl From<&crate::uci::SubnetId> for SubnetId {
    fn from(uci_id: &crate::uci::SubnetId) -> Self {
        Self(uci_id.to_string())
    }
}

impl PartialEq<crate::uci::SubnetId> for SubnetId {
    fn eq(&self, other: &crate::uci::SubnetId) -> bool {
        if let Ok(current) = crate::uci::SubnetId::from_str(&self.0) {
            other.as_array().eq(current.as_array())
        } else {
            error!("Failed to parse the subnet id {} during comparison", self.0);
            false
        }
    }
}
