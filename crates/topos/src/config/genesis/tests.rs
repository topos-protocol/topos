use std::io::Read;
use std::path::PathBuf;

use libp2p::PeerId;
use rstest::fixture;
use rstest::rstest;

use super::Genesis;

macro_rules! test_case {
    ($fname:expr) => {
        concat!(env!("CARGO_MANIFEST_DIR"), "/assets/", $fname)
    };
}

#[fixture]
#[once]
pub fn genesis() -> Genesis {
    Genesis::new(test_case!("genesis.json").into())
}

#[rstest]
pub fn test_correct_validator_count(genesis: &Genesis) {
    assert_eq!(4, genesis.validator_count());
}

#[rstest]
pub fn test_parse_bootnodes(genesis: &Genesis) {
    let bootnodes = genesis.boot_peers();

    assert_eq!(4, bootnodes.len());
}

#[rstest]
pub fn test_parse_extra_data(genesis: &Genesis) {
    let extra_data = genesis.extra_data();

    let bytes = hex::decode(&extra_data[2..]).expect("Decoding failed");

    // Define sizes
    const VANITY_SIZE: usize = 32;
    const VALIDATOR_SIZE: usize = 20;

    // Split into vanity, RLP encoded validators, and seal sections
    let (_, remaining) = bytes.split_at(VANITY_SIZE);

    let rlp = rlp::Rlp::new(remaining);
    let validator_bytes = rlp
        .at(0)
        .expect("Failed to get RLP list")
        .data()
        .expect("Failed to get RLP data");

    // Expected number of validators
    let num_validators = validator_bytes.len() / VALIDATOR_SIZE;

    // Extract validators
    let mut validators: Vec<String> = Vec::new();
    for i in 0..num_validators {
        validators.push(format!(
            "0x{}",
            hex::encode(&validator_bytes[i * VALIDATOR_SIZE..(i + 1) * VALIDATOR_SIZE])
        ));
    }

    assert_eq!(14, validators.len());
}
