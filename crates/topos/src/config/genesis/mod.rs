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
}
