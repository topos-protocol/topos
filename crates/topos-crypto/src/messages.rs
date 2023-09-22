use ethers::signers::Signer;
use ethers::signers::{LocalWallet, WalletError};
use ethers::types::{RecoveryMessage, SignatureError};
use ethers::utils::hash_message;
use std::sync::Arc;
use thiserror::Error;

pub use ethers::types::{Address, Signature, H160};

#[derive(Error, Debug)]
pub enum MessageSignerError {
    #[error("Unable to parse private key")]
    PrivateKeyParsing,
}

#[derive(Debug)]
pub struct MessageSigner {
    pub public_address: Address,
    wallet: LocalWallet,
}

impl MessageSigner {
    pub fn new(private_key: &str) -> Result<Arc<Self>, MessageSignerError> {
        let wallet: LocalWallet = private_key
            .parse()
            .map_err(|_| MessageSignerError::PrivateKeyParsing)?;

        Ok(Arc::new(Self {
            public_address: wallet.address(),
            wallet,
        }))
    }

    pub fn sign_message(&self, payload: &[u8]) -> Result<Signature, WalletError> {
        let hash = hash_message(payload);

        LocalWallet::sign_hash(&self.wallet, hash)
    }

    pub fn verify_signature(
        &self,
        signature: Signature,
        payload: &[u8],
        public_key: Address,
    ) -> Result<(), SignatureError> {
        let message: RecoveryMessage = payload.into();

        signature.verify(message, public_key)
    }
}
