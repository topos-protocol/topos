use ethers::signers::{LocalWallet, WalletError};
use ethers::types::{Address, RecoveryMessage, Signature, SignatureError};
use ethers::utils::hash_message;

pub fn sign_message(
    certificate_id: &[u8],
    validator_id: &[u8],
    wallet: &LocalWallet,
) -> Result<Signature, WalletError> {
    let mut message: Vec<u8> = Vec::with_capacity(certificate_id.len() + validator_id.len());

    message.extend_from_slice(certificate_id);
    message.extend_from_slice(validator_id);

    let hash = hash_message(message.as_slice());

    println!("hash: {:#?}", hash);

    LocalWallet::sign_hash(wallet, hash)
}

pub fn verify_signature(
    signature: Signature,
    certificate_id: &[u8],
    validator_id: &[u8],
    public_key: Address,
) -> Result<(), SignatureError> {
    let mut message: Vec<u8> = Vec::with_capacity(certificate_id.len() + validator_id.len());

    message.extend_from_slice(certificate_id);
    message.extend_from_slice(validator_id);

    let hash = hash_message(message.as_slice());

    println!("hash: {:#?}", hash);
    let message: RecoveryMessage = hash.into();

    signature.verify(message, public_key)
}
