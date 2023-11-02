use super::v1::EcdsaSignature;
use topos_crypto::messages::U256;

impl From<EcdsaSignature> for topos_crypto::messages::Signature {
    fn from(proto: EcdsaSignature) -> Self {
        topos_crypto::messages::Signature {
            r: U256::from_big_endian(&proto.r),
            s: U256::from_big_endian(&proto.s),
            v: proto.v,
        }
    }
}

impl From<topos_crypto::messages::Signature> for EcdsaSignature {
    fn from(other: topos_crypto::messages::Signature) -> Self {
        let mut ecdsa_signature = EcdsaSignature {
            r: vec![0; 32],
            s: vec![0; 32],
            v: 0,
        };
        other.r.to_big_endian(&mut ecdsa_signature.r);
        other.s.to_big_endian(&mut ecdsa_signature.s);
        ecdsa_signature.v = other.v;

        ecdsa_signature
    }
}
