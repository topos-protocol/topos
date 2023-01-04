//! Universal Certificate Interface
//!
//! Data structures to support Certificates' exchange

use keccak_hash::keccak_256;
use serde::{Deserialize, Serialize};
use std::borrow::BorrowMut;
use std::hash::Hash;
use std::time;
use thiserror::Error;

pub type CertificateId = [u8; 32];
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
    pub source_subnet_id: SubnetId,
    pub prev_id: CertificateId,
    //pub proof: StarkProof,
    //pub signature: Frost,
    pub calls: Vec<CrossChainTransaction>,
    pub id: CertificateId,
}

pub type DigestCompressed = Vec<CertificateId>; // TODO: optimize cmp to hash of sorted set of hashes

impl Certificate {
    pub fn new(
        prev: CertificateId,
        source_subnet_id: SubnetId,
        calls: Vec<CrossChainTransaction>,
    ) -> Result<Certificate, Box<dyn std::error::Error>> {
        let mut cert = Certificate {
            source_subnet_id,
            prev_id: prev,
            calls,
            id: [0; 32],
        };

        cert.id = calculate_keccak256(&cert)?;
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
    buffer.extend_from_slice(&certificate.prev_id[..]);
    buffer.extend_from_slice(bincode::serialize(&certificate.calls)?.as_slice());
    let mut hash = [0u8; 32];
    keccak_256(buffer.borrow_mut(), &mut hash);
    Ok(hash)
}

/// Cross chain transaction data definition
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum CrossChainTransactionData {
    AssetTransfer {
        sender: Address,
        receiver: Address,
        symbol: String,
        amount: Amount,
    },
    ContractCall {
        source_contract_addr: Address,
        target_contract_addr: Address,
        payload: Vec<u8>,
    },
    ContractCallWithToken {
        source_contract_addr: Address,
        target_contract_addr: Address,
        payload: Vec<u8>,
        symbol: String,
        amount: Amount,
    },
}

/// Cross chain txn
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub struct CrossChainTransaction {
    pub target_subnet_id: SubnetId,
    pub transaction_data: CrossChainTransactionData,
}
