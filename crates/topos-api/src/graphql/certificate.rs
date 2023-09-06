use async_graphql::{InputObject, SimpleObject};
use serde::{Deserialize, Serialize};

use super::subnet::SubnetId;

#[derive(Debug, Default, Serialize, Deserialize, InputObject)]
pub struct CertificateId {
    pub value: String,
}

#[derive(Debug, Default, Serialize, Deserialize, SimpleObject)]
#[serde(rename_all = "camelCase")]
pub struct Certificate {
    pub id: String,
    pub prev_id: String,
    pub proof: String,
    pub signature: String,
    pub source_subnet_id: String,
    pub state_root: String,
    pub target_subnets: Vec<SubnetId>,
    pub tx_root_hash: String,
    pub receipts_root_hash: String,
    pub verifier: u32,
}

impl From<topos_uci::Certificate> for Certificate {
    fn from(uci_cert: topos_uci::Certificate) -> Self {
        Self {
            id: uci_cert.id.to_string(),
            prev_id: uci_cert.prev_id.to_string(),
            proof: hex::encode(&uci_cert.proof),
            signature: hex::encode(&uci_cert.signature),
            source_subnet_id: uci_cert.source_subnet_id.to_string(),
            state_root: hex::encode(uci_cert.state_root),
            target_subnets: uci_cert
                .target_subnets
                .into_iter()
                .map(SubnetId::from)
                .collect(),
            tx_root_hash: hex::encode(uci_cert.tx_root_hash),
            receipts_root_hash: "0x".to_string() + &hex::encode(uci_cert.receipts_root_hash),
            verifier: uci_cert.verifier,
        }
    }
}

impl From<topos_uci::SubnetId> for SubnetId {
    fn from(uci_id: topos_uci::SubnetId) -> Self {
        SubnetId {
            value: uci_id.to_string(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::CertificateId;

    const CERTIFICATE_ID_WITH_PREFIX: &str =
        "0x11db8713a79c41625f4bb2221bd43ac4766fff23e78f82212f48713a6768e76a";
    const CERTIFICATE_ID_WITHOUT_PREFIX: &str =
        "11db8713a79c41625f4bb2221bd43ac4766fff23e78f82212f48713a6768e76a";
    const MALFORMATTED_CERTIFICATE_ID: &str = "invalid_hex_string";

    #[test]
    fn convert_cert_id_string_with_prefix() {
        let certificate_id1: CertificateId = CERTIFICATE_ID_WITH_PREFIX.to_string().into();

        assert_eq!(
            &certificate_id1.id[..],
            &[
                0x11, 0xdb, 0x87, 0x13, 0xa7, 0x9c, 0x41, 0x62, 0x5f, 0x4b, 0xb2, 0x22, 0x1b, 0xd4,
                0x3a, 0xc4, 0x76, 0x6f, 0xff, 0x23, 0xe7, 0x8f, 0x82, 0x21, 0x2f, 0x48, 0x71, 0x3a,
                0x67, 0x68, 0xe7, 0x6a
            ][..]
        )
    }
}
