use ethers::signers::Signer;
use ethers::signers::{LocalWallet, WalletError};
use ethers::types::{Address, RecoveryMessage, Signature, SignatureError};
use ethers::utils::hash_message;
use std::sync::Arc;

#[derive(Debug)]
pub struct MessageSigner {
    pub public_address: Address,
    wallet: LocalWallet,
}

impl MessageSigner {
    pub fn new(private_key: &str) -> Arc<Self> {
        let wallet: LocalWallet = private_key
            .parse()
            .expect("Cannot create wallet for private key");
        Arc::new(Self {
            public_address: wallet.address(),
            wallet,
        })
    }

    pub fn sign_message(
        &self,
        certificate_id: &[u8],
        validator_id: &[u8],
    ) -> Result<Signature, WalletError> {
        let mut message: Vec<u8> = Vec::with_capacity(certificate_id.len() + validator_id.len());
        message.extend_from_slice(certificate_id);
        message.extend_from_slice(validator_id);

        let hash = hash_message(message.as_slice());

        LocalWallet::sign_hash(&self.wallet, hash)
    }

    pub fn verify_signature(
        &self,
        signature: Signature,
        certificate_id: &[u8],
        validator_id: &[u8],
    ) -> Result<(), SignatureError> {
        let mut message: Vec<u8> = Vec::with_capacity(certificate_id.len() + validator_id.len());
        message.extend_from_slice(certificate_id);
        message.extend_from_slice(validator_id);

        let hash = hash_message(message.as_slice());
        let message: RecoveryMessage = hash.into();

        signature.verify(message, self.public_address)
    }
}
