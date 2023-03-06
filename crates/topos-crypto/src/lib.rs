use thiserror::Error;

pub mod hash;
pub mod keys;
pub mod keystore;
pub mod signatures;

#[derive(Debug, Error)]
pub enum Error {
    #[error("Keystore error: {0}")]
    KeystoreError(#[from] eth_keystore::KeystoreError),

    #[error("Keystore file io error: {0}")]
    KeystoreFileError(#[from] std::io::Error),

    #[error("Invalid key error: {0}")]
    InvalidKeyError(String),

    #[error("Eliptic curve error: {0}")]
    Secp256k1Error(#[from] secp256k1::Error),

    #[error("Invalid signature: {0}")]
    InvalidSignature(String),
}
