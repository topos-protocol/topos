use crate::*;
use serde::{Deserialize, Serialize};
use std::borrow::Borrow;
use std::fmt::Debug;

/// Certificate - main exchange item
#[derive(Clone, Default, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub struct Certificate {
    pub id: CertificateId,
    pub prev_id: CertificateId,
    pub source_subnet_id: SubnetId,
    pub state_root: StateRoot,
    pub tx_root_hash: TxRootHash,
    pub target_subnets: Vec<SubnetId>,
    pub verifier: u32,
    pub proof: StarkProof,
    pub signature: Frost,
}

impl AsRef<Certificate> for Certificate {
    fn as_ref(&self) -> &Self {
        self
    }
}

impl Debug for Certificate {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Certificate")
            .field("id", &self.id.to_string())
            .field("prev_id", &self.prev_id.to_string())
            .field("source_subnet_id", &self.source_subnet_id.to_string())
            .field(
                "state_root",
                &("0x".to_string() + &hex::encode(self.state_root)),
            )
            .field(
                "tx_root_hash",
                &("0x".to_string() + &hex::encode(self.tx_root_hash)),
            )
            .field(
                "target_subnets",
                &self
                    .target_subnets
                    .iter()
                    .map(|ts| ts.to_string())
                    .collect::<Vec<_>>(),
            )
            .field("verifier", &self.verifier)
            .field("proof", &("0x".to_string() + &hex::encode(&self.proof)))
            .field(
                "signature",
                &("0x".to_string() + &hex::encode(&self.signature)),
            )
            .finish()
    }
}

impl Certificate {
    pub fn new<P: Into<CertificateId>>(
        prev: P,
        source_subnet_id: SubnetId,
        state_root: StateRoot,
        tx_root_hash: TxRootHash,
        target_subnets: &[SubnetId],
        verifier: u32,
        proof: Vec<u8>,
    ) -> Result<Certificate, Box<dyn std::error::Error>> {
        let mut cert = Certificate {
            id: [0; 32].into(),
            prev_id: prev.into(),
            source_subnet_id,
            state_root,
            tx_root_hash,
            target_subnets: target_subnets.into(),
            verifier,
            proof,
            signature: Default::default(),
        };

        cert.id = Self::calculate_cert_id(&cert)?.into();
        Ok(cert)
    }

    pub fn check_signature(&self) -> Result<(), Error> {
        std::thread::sleep(DUMMY_FROST_VERIF_DELAY);
        Ok(())
    }

    pub fn check_proof(&self) -> Result<(), Error> {
        std::thread::sleep(DUMMY_STARK_DELAY);
        Ok(())
    }

    /// Signs the hash of the certificate payload
    pub fn update_signature(&mut self, private_key: &[u8]) -> Result<(), Error> {
        self.signature =
            topos_crypto::signatures::sign(private_key, self.get_payload().as_slice())?;
        Ok(())
    }

    /// Get byte payload of the certificate
    /// Excludes frost signature
    pub fn get_payload(&self) -> Vec<u8> {
        let mut buffer = Vec::new();
        buffer.extend(self.id.as_array().as_ref());
        buffer.extend_from_slice(self.prev_id.as_array().as_ref());
        buffer.extend_from_slice(self.source_subnet_id.as_array().as_ref());
        buffer.extend_from_slice(self.state_root.as_ref());
        buffer.extend_from_slice(self.tx_root_hash.as_ref());
        for target_subnet in &self.target_subnets {
            buffer.extend_from_slice(target_subnet.as_array().as_ref());
        }
        buffer.extend(self.verifier.to_be_bytes().as_ref());
        buffer.extend(self.proof.as_slice());
        buffer
    }

    // To get unique id, calculate certificate id of certificate object using keccak256,
    // excluding cert_id and signature fields
    fn calculate_cert_id(certificate: &Certificate) -> Result<[u8; 32], Error> {
        let mut buffer = Vec::new();
        buffer.extend_from_slice(certificate.prev_id.as_array().as_ref());
        buffer.extend_from_slice(certificate.source_subnet_id.as_array().as_ref());
        buffer.extend_from_slice(certificate.state_root.as_ref());
        buffer.extend_from_slice(certificate.tx_root_hash.as_ref());
        for target_subnet in &certificate.target_subnets {
            buffer.extend_from_slice(target_subnet.as_array().as_ref());
        }
        buffer.extend_from_slice(certificate.verifier.to_be_bytes().as_ref());
        buffer.extend_from_slice(certificate.proof.as_ref());
        let hash = topos_crypto::hash::calculate_hash(buffer.borrow());
        Ok(hash)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    const PREV_CERTIFICATE_ID: CertificateId = CertificateId::from_array([1u8; 32]);
    const TARGET_SUBNET_ID: SubnetId = SubnetId::from_array([3u8; 32]);
    const STATE_ROOT: StateRoot = [4u8; 32];
    const TX_ROOT_HASH: TxRootHash = [5u8; 32];
    const PRIVATE_TEST_KEY: &str =
        "5fb92d6e98884f76de468fa3f6278f8807c48bebc13595d45af5bdc4da702133";

    fn generate_dummy_cert(signing_key: &[u8]) -> Certificate {
        let public_key =
            topos_crypto::keys::derive_public_key(signing_key).expect("valid public key");
        let source_subnet_it: [u8; 32] = public_key[1..33].try_into().unwrap();

        Certificate::new(
            PREV_CERTIFICATE_ID,
            source_subnet_it.into(),
            STATE_ROOT,
            TX_ROOT_HASH,
            &[TARGET_SUBNET_ID],
            2,
            Default::default(),
        )
        .expect("Dummy certificate")
    }

    #[test]
    fn certificate_signatures() {
        let private_test_key = hex::decode(PRIVATE_TEST_KEY).unwrap();

        let mut dummy_cert = generate_dummy_cert(&private_test_key);
        dummy_cert
            .update_signature(private_test_key.as_slice())
            .expect("valid signature update");

        topos_crypto::signatures::verify(
            &dummy_cert.source_subnet_id.to_secp256k1_public_key(),
            dummy_cert.get_payload().as_slice(),
            dummy_cert.signature.as_slice(),
        )
        .expect("valid signature check")
    }

    #[test]
    #[should_panic]
    fn signature_verification_failed_corrupt_data() {
        let private_test_key = hex::decode(PRIVATE_TEST_KEY).unwrap();
        let mut dummy_cert = generate_dummy_cert(&private_test_key);

        dummy_cert
            .update_signature(private_test_key.as_slice())
            .expect("valid signature update");

        dummy_cert.state_root[0] = 0xff;

        let public_key = topos_crypto::keys::derive_public_key(private_test_key.as_slice())
            .expect("valid public key");

        topos_crypto::signatures::verify(
            &public_key,
            dummy_cert.get_payload().as_slice(),
            dummy_cert.signature.as_slice(),
        )
        .expect("invalid valid signature check")
    }

    #[test]
    #[should_panic]
    fn signature_verification_failed_invalid_public_key() {
        let private_test_key = hex::decode(PRIVATE_TEST_KEY).unwrap();
        let mut dummy_cert = generate_dummy_cert(&private_test_key);

        dummy_cert
            .update_signature(private_test_key.as_slice())
            .expect("valid signature update");

        dummy_cert.state_root[0] = 0xff;

        let mut public_key = topos_crypto::keys::derive_public_key(private_test_key.as_slice())
            .expect("valid public key");
        public_key[3] = 0xff;

        topos_crypto::signatures::verify(
            &dummy_cert.source_subnet_id.to_secp256k1_public_key(),
            dummy_cert.get_payload().as_slice(),
            dummy_cert.signature.as_slice(),
        )
        .expect("invalid valid signature check")
    }
}
