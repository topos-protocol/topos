//! Universal Certificate Interface
//!
//! Data structures to support Certificates' exchange

use serde::{Deserialize, Serialize};
use std::cmp::Ordering;
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::time;

pub type CertificateId = String;
pub type SubnetId = String;
pub type StarkProof = Vec<u8>;
pub type Frost = Vec<u8>;
pub type Address = String;
pub type Amount = ethereum_types::U256;

/// Heavily checked on the gossip, so not abstracted
const DUMMY_FROST_VERIF_DELAY: time::Duration = time::Duration::from_millis(0);

/// Zero second to abstract it by considering having a great machine
const DUMMY_STARK_DELAY: time::Duration = time::Duration::from_millis(0);

/// Certificate - main exchange item
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Certificate {
    pub source_subnet_id: SubnetId,
    pub cert_id: CertificateId,
    pub prev_cert_id: CertificateId,
    //pub proof: StarkProof,
    //pub signature: Frost,
    pub calls: Vec<CrossChainTransaction>,
}

/// TODO: separate contents cert for one subnet from the TCE wrappe
/// (e.g., containing subnet_id in it)
/// pub struct Certificate(CoreCertData, SubnetId, Signature)
/// pub struct CoreCertData {
///   index: relative to the subnet
///   prev_cert, calls, proof
/// }

pub type DigestCompressed = Vec<CertificateId>; // TODO: optimize cmp to hash of sorted set of hashes

impl Certificate {
    pub fn new(
        prev: CertificateId,
        source: SubnetId,
        calls: Vec<CrossChainTransaction>,
    ) -> Certificate {
        let mut cert = Certificate {
            cert_id: 0.to_string(),
            prev_cert_id: prev,
            source_subnet_id: source,
            calls,
        };

        cert.cert_id = calculate_hash(&cert);
        cert
    }

    pub fn check_signature(&self) -> Result<(), CertificateCheckingError> {
        std::thread::sleep(DUMMY_FROST_VERIF_DELAY);
        Ok(())
    }

    pub fn check_proof(&self) -> Result<(), CertificateCheckingError> {
        std::thread::sleep(DUMMY_STARK_DELAY);
        Ok(())
    }
}

pub enum CertificateCheckingError {}

fn calculate_hash<T: Hash>(t: &T) -> String {
    let mut s = DefaultHasher::new();
    t.hash(&mut s);
    s.finish().to_string()
}

impl Hash for Certificate {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.prev_cert_id.hash(state);
        self.source_subnet_id.hash(state);
        //self.proof.hash(state);
        self.calls.hash(state);
    }
}

impl Eq for Certificate {}
impl PartialEq for Certificate {
    fn eq(&self, other: &Self) -> bool {
        self.cert_id == other.cert_id
    }
}

impl Ord for Certificate {
    fn cmp(&self, other: &Self) -> Ordering {
        self.cert_id.cmp(&other.cert_id)
    }
}

impl PartialOrd for Certificate {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

/// Cross chain transaction data definition
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum CrossChainTransactionData {
    AssetTransfer { asset_id: String, amount: Amount },
    FunctionCall { data: Vec<u8> },
}

/// Cross chain txn
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub struct CrossChainTransaction {
    pub target_subnet_id: SubnetId,
    pub sender_addr: Address,
    pub recipient_addr: Address,
    pub transaction_data: CrossChainTransactionData,
}
