use std::io::Write;

pub const TEST_KEYSTORE_FILE_NAME: &str = "test-keystore.json";
pub const TEST_KEYSTORE_FILE_PASSWORD: &str = "123";

pub const TEST_KEYSTORE_DATA: &str = r#"{"crypto":{"cipher":"aes-128-ctr","cipherparams":{"iv":"479a498cce1c80a0ac51dd183f9b96ef"},"ciphertext":"ce40d0bab943352214fbd7923310ba607489e76bf2622ffebea558cbfb27c77b","kdf":"scrypt","kdfparams":{"dklen":32,"n":8192,"p":1,"r":8,"salt":"395438174df838577ec499d71ad5fe67ced8cf09e8096930619a6f427abf5cf8"},"mac":"5a41fed1220c6b78068f5f15d1009f5914abacf4eb9c36cb9aebc258f50b233b"},"id":"6726da52-1ae5-4f65-9469-04c2da8a35d6","version":3}"#;

pub fn generate_test_keystore_file() -> Result<String, Box<dyn std::error::Error>> {
    let keystore_file_path = std::env::temp_dir().join(TEST_KEYSTORE_FILE_NAME);
    let mut keystore_file = std::fs::File::create(keystore_file_path.clone())?;
    writeln!(&mut keystore_file, "{}", TEST_KEYSTORE_DATA)?;
    Ok(keystore_file_path
        .as_os_str()
        .to_str()
        .expect("valid keystore file path")
        .to_string())
}
