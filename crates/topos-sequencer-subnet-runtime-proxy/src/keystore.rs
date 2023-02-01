use crate::Error;
/// Module for handling local topos node keystore
use std::path::Path;

pub const SUBNET_NODE_VALIDATOR_KEY_FILE_PATH: &str = "/consensus/validator.key";

pub fn get_private_key(
    file_name: &std::path::PathBuf,
    password: Option<String>,
) -> Result<Vec<u8>, Error> {
    let keypath = Path::new(file_name);
    let private_key = if let Some(password) = password {
        // Encrypted keystore in ethereum wallet format
        eth_keystore::decrypt_key(keypath, password)?
    } else {
        let key = std::fs::read_to_string(keypath)?.trim().to_string();
        hex::decode(&key).map_err(|e| Error::InvalidKeyError {
            message: e.to_string(),
        })?
    };

    Ok(private_key)
}
