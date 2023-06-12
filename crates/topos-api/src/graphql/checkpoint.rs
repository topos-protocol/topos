use async_graphql::InputObject;
use serde::{Deserialize, Serialize};

use crate::graphql::errors::GraphQLServerError;
use topos_uci::{self, SUBNET_ID_LENGTH};

#[derive(Debug, Default, Serialize, Deserialize, InputObject)]
pub struct CertificateId {
    pub value: String,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, InputObject)]
pub struct InputSubnetId {
    pub value: String,
}

#[derive(Debug, Default, Serialize, Deserialize, InputObject)]
pub struct SourceStreamPosition {
    pub source_subnet_id: InputSubnetId,
    pub position: u64,
    pub certificate_id: Option<CertificateId>,
}

#[derive(Debug, Default, Serialize, Deserialize, InputObject)]
pub struct SourceCheckpoint {
    pub source_subnet_ids: Vec<InputSubnetId>,
    pub positions: Vec<SourceStreamPosition>,
}

impl InputSubnetId {
    pub fn to_uci_subnet_id(self) -> Result<topos_uci::SubnetId, GraphQLServerError> {
        // Remove the leading "0x" from the hexadecimal string
        let hex_string = self.value.trim_start_matches("0x");

        // Decode the hexadecimal string into a vector of bytes
        let bytes = hex::decode(hex_string).map_err(|_| GraphQLServerError::ParseSubnetId)?;

        // Convert the vector of bytes into a fixed-size array
        let mut array: [u8; SUBNET_ID_LENGTH] = [0; SUBNET_ID_LENGTH];
        array.copy_from_slice(&bytes[..SUBNET_ID_LENGTH]);

        Ok(topos_uci::SubnetId::from_array(array))
    }
}
