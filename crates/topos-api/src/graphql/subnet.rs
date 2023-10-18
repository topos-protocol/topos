use async_graphql::{InputObject, SimpleObject};
use serde::{Deserialize, Serialize};
use std::str::FromStr;
use tracing::{error, warn};

use super::errors::GraphQLServerError;

#[derive(
    Clone, Debug, Default, Serialize, Deserialize, SimpleObject, InputObject, PartialEq, Eq,
)]
#[graphql(input_name = "SubnetIdInput")]
pub struct SubnetId {
    pub value: String,
}

impl TryFrom<&SubnetId> for topos_uci::SubnetId {
    type Error = GraphQLServerError;

    fn try_from(value: &SubnetId) -> Result<Self, Self::Error> {
        Self::from_str(value.value.as_str()).map_err(|e| {
            error!("Failed to convert SubnetId from GraphQL input {e:?}");
            GraphQLServerError::ParseDataConnector
        })
    }
}

impl PartialEq<topos_uci::SubnetId> for SubnetId {
    fn eq(&self, other: &topos_uci::SubnetId) -> bool {
        if let Ok(current) = topos_uci::SubnetId::from_str(&self.value) {
            other.as_array().eq(current.as_array())
        } else {
            warn!("Unexpected parsing error for subnet id during comparaison");
            false
        }
    }
}
