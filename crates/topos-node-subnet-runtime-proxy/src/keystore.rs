/// Module for handling local topos node keystore
use clap::Parser;
use std::path::Path;

pub fn get_private_key(
    file_name: &str,
    password: &str,
) -> Result<Vec<u8>, eth_keystore::KeystoreError> {
    let keypath = Path::new(file_name);
    let private_key = eth_keystore::decrypt_key(keypath, password)?;
    Ok(private_key)
}

/// Topos node keystor util parameters
#[derive(Debug, Parser, Clone)]
#[clap(name = "Topos Node Keystore")]
struct Args {
    // Create keystore. If not set read keystore
    #[clap(short, long, action)]
    new: bool,
    // New keystore filename
    #[clap(short, long, default_value = "topos-node-keystore.json")]
    pub filename: String,
    // New keystore password
    #[clap(short, long)]
    pub password: String,
    // Hex encoded private key, e.g. "0x0000000000000000000000000000000000000000000000000000000000000000"
    #[clap(short, long)]
    pub key: Option<String>,
}

pub fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Args::parse();
    if args.new {
        println!("Creating new keystore {}", args.filename);
        let path = Path::new(&args.filename);
        let mut rng = rand::thread_rng();
        // Parse private key
        let hex_encoded_key = args.key.expect("Valid private key missing");
        if &hex_encoded_key[0..2] != "0x" || hex_encoded_key.len() != 66 {
            eprintln!("Error, key must be Ethereum hex encoded string starting with 0x")
        }
        let private_key = hex::decode(&hex_encoded_key[2..]).expect("expected hex encoded string");
        let private_key: [u8; 32] = (&private_key[..])
            .try_into()
            .expect("invalid key, expected 32 bytes key");
        // Encrypt private key and store to keystore file
        eth_keystore::encrypt_key(
            path.parent().expect("valid parent directory"),
            &mut rng,
            private_key,
            &args.password,
            Some(path.file_name().expect("valid filename").to_str().unwrap()),
        )?;
    } else {
        println!("Extracting private key from keystore {}", args.filename);
        let keypath = Path::new(&args.filename);
        let private_key = eth_keystore::decrypt_key(keypath, args.password)?;
        println!(
            "Extracted private key:0x{}",
            hex::encode(&private_key).replace('\"', "")
        );
    }
    Ok(())
}
