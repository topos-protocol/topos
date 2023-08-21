use std::path::PathBuf;

use super::genesis::Genesis;
use libp2p::PeerId;
use rstest::fixture;
use rstest::rstest;

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
