///
/// Protobuf generated/native Rust structures related conversions for GRPC API
///
use crate::uci::v1 as proto_v1;

impl From<proto_v1::Certificate> for topos_uci::Certificate {
    fn from(certificate: proto_v1::Certificate) -> Self {
        topos_uci::Certificate {
            source_subnet_id: certificate
                .source_subnet_id
                .expect("valid source subnet id")
                .value
                .try_into()
                .expect("expected valid source subnet id with correct length"),
            id: certificate
                .id
                .expect("valid certificate id")
                .value
                .try_into()
                .expect("expected valid certificate id with correct length"),
            prev_id: certificate
                .prev_id
                .expect("valid previous certificate id")
                .value
                .try_into()
                .expect("expected valid previous certificate id with correct length"),
            target_subnets: certificate
                .target_subnets
                .into_iter()
                .map(|target_subnet| target_subnet.into())
                .collect(),
        }
    }
}

impl From<topos_uci::Certificate> for proto_v1::Certificate {
    fn from(certificate: topos_uci::Certificate) -> Self {
        proto_v1::Certificate {
            source_subnet_id: Some(crate::shared::v1::SubnetId {
                value: certificate.source_subnet_id.to_vec(),
            }),
            id: Some(crate::shared::v1::CertificateId {
                value: certificate.id.into(),
            }),
            prev_id: Some(crate::shared::v1::CertificateId {
                value: certificate.prev_id.into(),
            }),
            target_subnets: certificate
                .target_subnets
                .into_iter()
                .map(|target_subnet| target_subnet.into())
                .collect(),
        }
    }
}
