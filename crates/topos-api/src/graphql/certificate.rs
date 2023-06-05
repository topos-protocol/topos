use serde::{Deserialize, Serialize};
use async_graphql::SimpleObject;

#[derive(Debug, Serialize, Deserialize, SimpleObject)]
pub struct TargetSubnet {
    value: String,
}

#[derive(Debug, Serialize, Deserialize, SimpleObject)]
pub struct Certificate {
    pub prev_id: String,
    pub proof: String,
    pub signature : String,
    pub source_subnet_id: String,
    pub state_root: String,
    pub target_subnets: Vec<TargetSubnet>,
    pub tx_root_hash: String,
    pub verifier: u64,
}