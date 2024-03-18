use std::{str::FromStr, sync::Arc};

use rstest::fixture;
use topos_crypto::messages::MessageSigner;

#[fixture(key = "122f3ae6ade1fd136b292cea4f6243c7811160352c8821528547a1fe7c459daf")]
pub fn message_signer(key: &str) -> Arc<MessageSigner> {
    Arc::new(MessageSigner::from_str(key).unwrap())
}
