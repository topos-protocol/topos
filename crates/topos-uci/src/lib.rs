//! Universal Certificate Interface
//!
//! Data structures to support Certificates' exchange

pub use certificate_id::CertificateId;
use keccak_hash::keccak_256;
use serde::{Deserialize, Serialize};
use std::borrow::BorrowMut;
use std::hash::Hash;
use std::time;
use thiserror::Error;

mod certificate_id;

pub type SubnetId = [u8; 32];
pub type StarkProof = Vec<u8>;
pub type Frost = Vec<u8>;
pub type Address = [u8; 20];
pub type Amount = ethereum_types::U256;

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
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub struct Certificate {
    pub prev_id: CertificateId,
    pub source_subnet_id: SubnetId,
    //pub position: u64,
    //pub state_root: [u8; 32],
    //pub tx_root_hash: [u8; 32],
    pub target_subnets: Vec<SubnetId>,
    //pub proof: StarkProof,
    pub id: CertificateId,
    //pub signature: Frost,
}

pub type DigestCompressed = Vec<CertificateId>; // TODO: optimize cmp to hash of sorted set of hashes

impl Certificate {
    pub fn new<P: Into<CertificateId>>(
        prev: P,
        source_subnet_id: SubnetId,
        target_subnets: &[SubnetId],
    ) -> Result<Certificate, Box<dyn std::error::Error>> {
        let mut cert = Certificate {
            source_subnet_id,
            prev_id: prev.into(),
            target_subnets: target_subnets.into(),
            id: [0; 32].into(),
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
    buffer.extend_from_slice(&certificate.source_subnet_id[..]);
    buffer.extend_from_slice(&certificate.prev_id.as_array()[..]);
    buffer.extend_from_slice(bincode::serialize(&certificate.target_subnets)?.as_slice());
    let mut hash = [0u8; 32];
    keccak_256(buffer.borrow_mut(), &mut hash);
    Ok(hash)
}
