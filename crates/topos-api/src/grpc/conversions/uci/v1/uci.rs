//!
//! Protobuf generated/native Rust structures related conversions for GRPC API
//!
use crate::grpc::shared::v1_conversions_subnet::Error;
use crate::grpc::uci::v1 as proto_v1;

impl TryFrom<proto_v1::Certificate> for topos_uci::Certificate {
    type Error = Error;

    fn try_from(certificate: proto_v1::Certificate) -> Result<Self, Self::Error> {
        Ok(topos_uci::Certificate {
            prev_id: certificate
                .prev_id
                .ok_or(Error::MissingField("certificate.prev_id"))?
                .value
                .as_slice()
                .try_into()?,
            source_subnet_id: certificate
                .source_subnet_id
                .ok_or(Error::MissingField("certificate.source_subnet_id"))?
                .value
                .as_slice()
                .try_into()?,
            state_root: certificate
                .state_root
                .try_into()
                .map_err(|_| Error::InvalidStateRoot)?,
            tx_root_hash: certificate
                .tx_root_hash
                .try_into()
                .map_err(|_| Error::InvalidTxRootHash)?,
            receipts_root_hash: certificate
                .receipts_root_hash
                .try_into()
                .map_err(|_| Error::InvalidReceiptsRootHash)?,
            target_subnets: certificate
                .target_subnets
                .into_iter()
                .map(TryInto::try_into)
                .collect::<Result<Vec<topos_uci::SubnetId>, _>>()?,
            verifier: certificate.verifier,
            id: certificate
                .id
                .ok_or(Error::MissingField("certificate.id"))?
                .value
                .as_slice()
                .try_into()?,
            proof: certificate.proof.expect("valid proof").value,
            signature: certificate.signature.expect("valid frost signature").value,
        })
    }
}

impl From<topos_uci::Certificate> for proto_v1::Certificate {
    fn from(certificate: topos_uci::Certificate) -> Self {
        proto_v1::Certificate {
            prev_id: Some(crate::grpc::shared::v1::CertificateId {
                value: certificate.prev_id.into(),
            }),
            source_subnet_id: Some(crate::grpc::shared::v1::SubnetId {
                value: certificate.source_subnet_id.into(),
            }),
            state_root: certificate.state_root.to_vec(),
            tx_root_hash: certificate.tx_root_hash.to_vec(),
            receipts_root_hash: certificate.receipts_root_hash.to_vec(),
            verifier: certificate.verifier,
            target_subnets: certificate
                .target_subnets
                .into_iter()
                .map(|target_subnet| target_subnet.into())
                .collect(),
            id: Some(crate::grpc::shared::v1::CertificateId {
                value: certificate.id.into(),
            }),
            proof: Some(crate::grpc::shared::v1::StarkProof {
                value: certificate.proof,
            }),
            signature: Some(crate::grpc::shared::v1::Frost {
                value: certificate.signature,
            }),
        }
    }
}
