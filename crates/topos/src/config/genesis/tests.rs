use rstest::fixture;
use rstest::rstest;
use topos_tce_transport::ValidatorId;

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
    let bootnodes = genesis.boot_peers(None);

    assert_eq!(4, bootnodes.len());
}

#[rstest]
pub fn test_extract_validators(genesis: &Genesis) {
    let validators = genesis.validators();

    let first = ValidatorId::from("0x100d617e4392c02b31bdce650b26b6c0c3e04f95");
    let second = ValidatorId::from("0x92183cff18a1328e7d791d607589a15d9eee4bc4");
    let third = ValidatorId::from("0xb4973cdb10894d1d1547673bd758589034c2bba5");
    let fourth = ValidatorId::from("0xc16d83893cb61872206d4e271b813015d3242d94");

    assert_eq!(validators.len(), 4);
    assert_eq!(validators.get(&first), Some(&first));
    assert_eq!(validators.get(&second), Some(&second));
    assert_eq!(validators.get(&third), Some(&third));
    assert_eq!(validators.get(&fourth), Some(&fourth));
}
