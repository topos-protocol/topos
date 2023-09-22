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
