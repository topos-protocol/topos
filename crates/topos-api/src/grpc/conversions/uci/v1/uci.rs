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

#[test]
fn test_proto_uci_certificate_conversion_id_random_0x() {
    use crate::grpc::shared::v1::{CertificateId, Frost, StarkProof, SubnetId};
    let valid_cert = proto_v1::Certificate {
        prev_id: Some(CertificateId {
            value: vec![
                134, 103, 37, 44, 159, 78, 218, 73, 112, 17, 202, 189, 112, 180, 121, 0, 12, 128,
                186, 116, 161, 18, 122, 129, 75, 151, 144, 95, 63, 203, 218, 69,
            ],
        }),
        source_subnet_id: Some(SubnetId {
            value: vec![
                98, 139, 93, 91, 125, 115, 135, 224, 46, 222, 68, 33, 52, 2, 83, 179, 100, 2, 44,
                97, 103, 55, 128, 90, 14, 40, 56, 72, 66, 59, 0, 181,
            ],
        }),
        state_root: vec![
            145, 239, 242, 24, 12, 214, 83, 202, 223, 162, 240, 11, 146, 240, 28, 179, 163, 174,
            70, 6, 216, 40, 150, 1, 195, 33, 156, 132, 21, 43, 6, 236,
        ],
        tx_root_hash: vec![
            86, 232, 31, 23, 27, 204, 85, 166, 255, 131, 69, 230, 146, 192, 248, 110, 91, 72, 224,
            27, 153, 108, 173, 192, 1, 98, 47, 181, 227, 99, 180, 33,
        ],
        receipts_root_hash: vec![
            86, 232, 31, 23, 27, 204, 85, 166, 255, 131, 69, 230, 146, 192, 248, 110, 91, 72, 224,
            27, 153, 108, 173, 192, 1, 98, 47, 181, 227, 99, 180, 33,
        ],
        target_subnets: Vec::new(),
        verifier: 0,
        id: Some(CertificateId {
            value: vec![
                48, 120, 230, 118, 216, 103, 205, 65, 12, 143, 205, 166, 153, 107, 194, 94, 158,
                29, 135, 167, 231, 50, 238, 173, 96, 165, 27, 215, 255, 94, 18, 199,
            ],
        }),
        proof: Some(StarkProof { value: Vec::new() }),
        signature: Some(Frost {
            value: vec![
                76, 181, 52, 25, 163, 103, 87, 142, 229, 64, 163, 77, 11, 225, 135, 96, 181, 34,
                168, 13, 152, 69, 90, 202, 11, 235, 122, 214, 103, 26, 31, 109, 94, 117, 53, 83,
                195, 74, 47, 175, 189, 3, 134, 164, 186, 179, 73, 86, 202, 172, 213, 195, 160, 139,
                240, 230, 103, 81, 227, 99, 241, 130, 157, 188,
            ],
        }),
    };
    if let Err(e) = topos_uci::Certificate::try_from(valid_cert) {
        panic!("Unable to perform certificate conversion: {e}");
    };
}

#[test]
fn test_proto_uci_certificate_conversion_id_starts_with_0x() {
    use crate::grpc::shared::v1::{CertificateId, Frost, StarkProof, SubnetId};
    let mut prev_id = vec![b'0', b'x'];
    let id = "504b5d01948bc777ba1510ba92a901f516408e4b2a1a5b97fed719430acc9ec9";
    prev_id.append(
        &mut hex::decode("aac03cadfff6846c9ce72956eee2498011dd7b08689565d6f29e25c0a967ef14")
            .expect("Valid id"),
    );
    let valid_cert = proto_v1::Certificate {
        prev_id: Some(CertificateId { value: prev_id }),
        id: Some(CertificateId {
            value: hex::decode(id).expect("Valid id"),
        }),
        source_subnet_id: Some(SubnetId::from([0u8; 32])),
        state_root: [0u8; 32].to_vec(),
        tx_root_hash: [0u8; 32].to_vec(),
        receipts_root_hash: [0u8; 32].to_vec(),
        proof: Some(StarkProof { value: Vec::new() }),
        signature: Some(Frost { value: Vec::new() }),
        ..Default::default()
    };
    let cert: topos_uci::Certificate = match topos_uci::Certificate::try_from(valid_cert) {
        Ok(cert) => cert,
        Err(e) => {
            panic!("Unable to perform certificate conversion: {e}");
        }
    };
    println!(
        "First certificate converted prev_id={}, id={}",
        cert.prev_id, cert.id
    );

    let prev_id = "0xFF4b5d01948bc777ba1510ba92a901f516408e4b2a1a5b97fed719430acc9ec9"
        .to_string()
        .into_bytes();
    let id = "AA4b5d01948bc777ba1510ba92a901f516408e4b2a1a5b97fed719430acc9ec9"
        .to_string()
        .into_bytes();
    let valid_cert = proto_v1::Certificate {
        prev_id: Some(CertificateId { value: prev_id }),
        id: Some(CertificateId { value: id }),
        source_subnet_id: Some(SubnetId::from([0u8; 32])),
        state_root: [0u8; 32].to_vec(),
        tx_root_hash: [0u8; 32].to_vec(),
        receipts_root_hash: [0u8; 32].to_vec(),
        proof: Some(StarkProof { value: Vec::new() }),
        signature: Some(Frost { value: Vec::new() }),
        ..Default::default()
    };
    let cert: topos_uci::Certificate = match topos_uci::Certificate::try_from(valid_cert) {
        Ok(cert) => cert,
        Err(e) => {
            panic!("Unable to perform certificate conversion: {e}");
        }
    };

    println!(
        "Second certificate converted prev_id={}, id={}",
        cert.prev_id, cert.id
    );
}
