use crate::Error;
use secp256k1::{PublicKey, Secp256k1, SecretKey};

pub fn derive_public_key(private_key: &[u8]) -> Result<Vec<u8>, Error> {
    let secret_key =
        SecretKey::from_slice(private_key).map_err(|e| Error::InvalidKeyError(e.to_string()))?;
    Ok(PublicKey::from_secret_key(&Secp256k1::new(), &secret_key)
        .serialize()
        .to_vec())
}
