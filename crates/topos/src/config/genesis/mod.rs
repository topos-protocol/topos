use std::{fs, path::PathBuf};

use serde_json::Value;
use topos_p2p::{Multiaddr, PeerId};

#[cfg(test)]
pub(crate) mod tests;

/// From the Edge format
pub struct Genesis {
    pub path: PathBuf,
    pub json: Value,
}

impl Genesis {
    pub fn new(path: PathBuf) -> Self {
        let genesis_file = fs::File::open(&path).expect("opened file");

        let json: serde_json::Value =
            serde_json::from_reader(genesis_file).expect("genesis json parsed");

        Self { path, json }
    }

    // Considered as being the set of premined addresses for now
    // TODO: Parse properly genesis.extraData instead
    pub fn validator_count(&self) -> usize {
        self.json["genesis"]["alloc"]
            .as_object()
            .map_or(0, |v| v.len())
    }

    // TODO: parse directly with serde
    pub fn boot_peers(&self) -> Vec<(PeerId, Multiaddr)> {
        match self.json["bootnodes"].as_array() {
            Some(v) => v
                .iter()
                .map(|bootnode| {
                    let (multiaddr, peerid) =
                        bootnode.as_str().unwrap().rsplit_once("/p2p/").unwrap();
                    (peerid.parse().unwrap(), multiaddr.parse().unwrap())
                })
                .collect::<Vec<_>>(),
            None => Vec::default(),
        }
    }

    /// Parse the validators from the `extraData` field of the genesis file.
    /// The `extraData` is patted with 32 bytes, and the validators are RLP encoded.
    /// Each validator is 20 bytes, with a SEAL at the end of the whole list (8 bytes)
    #[allow(dead_code)]
    pub fn validators(&self) -> Vec<String> {
        let extra_data = self.json["genesis"]["extraData"]
            .as_str()
            .unwrap()
            .to_string();

        let bytes = hex::decode(&extra_data[2..]).expect("Decoding failed");

        // Define sizes
        const VANITY_SIZE: usize = 32;
        const VALIDATOR_SIZE: usize = 20;

        // Split into vanity, RLP encoded validators, and seal sections
        let (_vanity, remaining) = bytes.split_at(VANITY_SIZE);

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

        validators
    }
}
