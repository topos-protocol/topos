use std::io::Write;

pub const TEST_VALIDATOR_KEY_FILE_DATA: &str =
    r#"11eddfae7abe45531b3f18342c8062969323a7131d3043f1a33c40df74803cc7"#;

#[allow(dead_code)]
pub fn generate_test_subnet_data_dir() -> Result<std::path::PathBuf, Box<dyn std::error::Error>> {
    const TEST_VALIDATOR_SUBNET_DATA_DIR: &str = "/test_data";
    const TEST_VALIDATOR_KEY_FILE_NAME: &str = "/consensus/validator.key";
    let tmp = std::env::temp_dir().to_str().unwrap().to_string();
    let subnet_data_dir = std::path::PathBuf::from(tmp.clone() + TEST_VALIDATOR_SUBNET_DATA_DIR);
    let consensus_dir =
        std::path::PathBuf::from(tmp.clone() + TEST_VALIDATOR_SUBNET_DATA_DIR + "/consensus");
    let keystore_file_path = std::path::PathBuf::from(
        tmp + TEST_VALIDATOR_SUBNET_DATA_DIR + TEST_VALIDATOR_KEY_FILE_NAME,
    );
    std::fs::create_dir_all(consensus_dir)?;
    let mut keystore_file = std::fs::File::create(keystore_file_path.clone())?;
    writeln!(&mut keystore_file, "{}", TEST_VALIDATOR_KEY_FILE_DATA)?;
    Ok(subnet_data_dir)
}

pub fn generate_test_private_key() -> Vec<u8> {
    hex::decode(TEST_VALIDATOR_KEY_FILE_DATA).unwrap()
}
