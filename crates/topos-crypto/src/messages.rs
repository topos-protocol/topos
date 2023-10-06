use ethers::signers::Signer;
use ethers::signers::{LocalWallet, WalletError};
use ethers::types::{RecoveryMessage, SignatureError};
use ethers::utils::hash_message;
use std::str::FromStr;
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
    pub wallet: LocalWallet,
}

impl FromStr for MessageSigner {
    type Err = MessageSignerError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let decoded = hex::decode(s).map_err(|_| MessageSignerError::PrivateKeyParsing)?;

        Self::new(&decoded[..])
    }
}

impl MessageSigner {
    pub fn new(private_key: &[u8]) -> Result<Self, MessageSignerError> {
        let wallet: LocalWallet = LocalWallet::from_bytes(private_key)
            .map_err(|_| MessageSignerError::PrivateKeyParsing)?;

        Ok(Self {
            public_address: wallet.address(),
            wallet,
        })
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
