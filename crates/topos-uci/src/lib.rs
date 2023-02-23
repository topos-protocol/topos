//! Universal Certificate Interface
//!
//! Data structures to support Certificates' exchange

pub use certificate_id::CertificateId;
use keccak_hash::keccak_256;
use serde::{Deserialize, Serialize};
use std::borrow::BorrowMut;
use std::fmt::Debug;
use std::hash::Hash;
use std::time;
use thiserror::Error;

mod certificate_id;

pub type SubnetId = [u8; 32];
pub type StarkProof = Vec<u8>;
pub type Frost = Vec<u8>;
pub type Address = [u8; 20];
pub type Amount = ethereum_types::U256;
pub type StateRoot = [u8; 32];
pub type TxRootHash = [u8; 32];

/// Heavily checked on the gossip, so not abstracted
const DUMMY_FROST_VERIF_DELAY: time::Duration = time::Duration::from_millis(0);

/// Zero second to abstract it by considering having a great machine
const DUMMY_STARK_DELAY: time::Duration = time::Duration::from_millis(0);

#[derive(Debug, Error)]
pub enum Error {
    #[error("certificate validation error")]
    ValidationError,
    #[error("bincode error: {source}")]
    BincodeError {
        #[from]
        source: bincode::Error,
    },
}

/// Certificate - main exchange item
#[derive(Clone, Default, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub struct Certificate {
    pub prev_id: CertificateId,
    pub source_subnet_id: SubnetId,
    pub state_root: StateRoot,
    pub tx_root_hash: TxRootHash,
    pub target_subnets: Vec<SubnetId>,
    pub verifier: u32,
    pub id: CertificateId,
    pub proof: StarkProof,
    pub signature: Frost,
}

impl Debug for Certificate {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Certificate")
            .field("prev_id", &self.prev_id)
            .field("source_subnet_id", &hex::encode(self.source_subnet_id))
            .field("state_root", &&hex::encode(self.state_root))
            .field("tx_root_hash", &hex::encode(self.tx_root_hash))
            .field(
                "target_subnets",
                &self
                    .target_subnets
                    .iter()
                    .map(hex::encode)
                    .collect::<Vec<_>>(),
            )
            .field("verifier", &self.verifier)
            .field("id", &self.id)
            .field("proof", &self.proof)
            .field("signature", &self.signature)
            .finish()
    }
}

pub type DigestCompressed = Vec<CertificateId>; // TODO: optimize cmp to hash of sorted set of hashes

impl Certificate {
    pub fn new<P: Into<CertificateId>>(
        prev: P,
        source_subnet_id: SubnetId,
        state_root: StateRoot,
        tx_root_hash: TxRootHash,
        target_subnets: &[SubnetId],
        verifier: u32,
    ) -> Result<Certificate, Box<dyn std::error::Error>> {
        let mut cert = Certificate {
            prev_id: prev.into(),
            source_subnet_id,
            state_root,
            tx_root_hash,
            target_subnets: target_subnets.into(),
            verifier,
            id: [0; 32].into(),
            proof: Default::default(),
            signature: Default::default(),
        };

        cert.id = calculate_keccak256(&cert)?.into();
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
}

// Calculates hash of certificate object, excluding cert_id field
pub fn calculate_keccak256(certificate: &Certificate) -> Result<[u8; 32], Error> {
    let mut buffer = Vec::new();
    buffer.extend_from_slice(&certificate.prev_id.as_array()[..]);
    buffer.extend_from_slice(&certificate.source_subnet_id[..]);
    buffer.extend_from_slice(&certificate.state_root[..]);
    buffer.extend_from_slice(&certificate.tx_root_hash[..]);
    buffer.extend_from_slice(bincode::serialize(&certificate.target_subnets)?.as_slice());
    buffer.extend(&certificate.verifier.to_be_bytes()[..]);
    let mut hash = [0u8; 32];
    keccak_256(buffer.borrow_mut(), &mut hash);
    Ok(hash)
}
