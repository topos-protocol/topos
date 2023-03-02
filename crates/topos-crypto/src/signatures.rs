use crate::Error;
use secp256k1::{Message, PublicKey, Secp256k1, SecretKey};

pub fn sign(private_key: &[u8], data: &[u8]) -> Result<Vec<u8>, crate::Error> {
    let secp = Secp256k1::new();
    let secret_key =
        SecretKey::from_slice(private_key).map_err(|e| Error::InvalidKeyError(e.to_string()))?;
    let hash = crate::hash::calculate_hash(data);
    let message = Message::from_slice(&hash).map_err(Error::Secp256k1Error)?;
    let signature = secp.sign_ecdsa(&message, &secret_key);

    Ok(signature.serialize_compact().to_vec())
}

pub fn verify(public_key: &[u8], data: &[u8], signature: &[u8]) -> Result<(), crate::Error> {
    let secp = Secp256k1::new();
    let public_key =
        PublicKey::from_slice(public_key).map_err(|e| Error::InvalidKeyError(e.to_string()))?;
    let signature = secp256k1::ecdsa::Signature::from_compact(signature)
        .map_err(|e| Error::InvalidSignature(e.to_string()))?;

    let hash = crate::hash::calculate_hash(data);
    let message = Message::from_slice(&hash).map_err(Error::Secp256k1Error)?;
    secp.verify_ecdsa(&message, &signature, &public_key)
        .map_err(|e| Error::InvalidSignature(e.to_string()))
}
