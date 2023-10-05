//! Universal Certificate Interface
//!
//! Data structures to support Certificates' exchange

pub use certificate::Certificate;
pub use certificate_id::CertificateId;
pub use subnet_id::SubnetId;

use std::fmt::Debug;
use std::time;
use thiserror::Error;

mod certificate;
mod certificate_id;
mod subnet_id;

pub const CERTIFICATE_ID_LENGTH: usize = 32;
pub const HEX_CERTIFICATE_ID_LENGTH: usize = 64;
pub const SUBNET_ID_LENGTH: usize = 32;
pub use certificate_id::INITIAL_CERTIFICATE_ID;

pub type StarkProof = Vec<u8>;
pub type Frost = Vec<u8>;
pub type Address = [u8; 20];
pub type Amount = ethereum_types::U256;
pub type StateRoot = [u8; 32];
pub type TxRootHash = [u8; 32];
pub type ReceiptsRootHash = [u8; 32];

/// Heavily checked on the gossip, so not abstracted
const DUMMY_FROST_VERIF_DELAY: time::Duration = time::Duration::from_millis(0);

/// Zero second to abstract it by considering having a great machine
const DUMMY_STARK_DELAY: time::Duration = time::Duration::from_millis(0);

#[derive(Debug, Error)]
pub enum Error {
    #[error("certificate validation error: {0}")]
    ValidationError(String),

    #[error("topos crypto error: (0)")]
    CryptoError(#[from] topos_crypto::Error),
}
