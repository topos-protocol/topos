use std::net::SocketAddr;

use serde::{Deserialize, Serialize};
use topos_p2p::Multiaddr;

use super::DEFAULT_IP;

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "kebab-case")]
pub struct P2PConfig {
    /// List of multiaddresses to listen for incoming connections
    #[serde(default = "default_listen_addresses")]
    pub listen_addresses: Vec<Multiaddr>,
    /// List of multiaddresses to advertise to the network
    #[serde(default = "default_public_addresses")]
    pub public_addresses: Vec<Multiaddr>,

    #[serde(skip)]
    pub is_bootnode: bool,
}

impl Default for P2PConfig {
    fn default() -> Self {
        Self {
            listen_addresses: default_listen_addresses(),
            public_addresses: default_public_addresses(),
            is_bootnode: false,
        }
    }
}

const fn default_libp2p_api_addr() -> SocketAddr {
    SocketAddr::V4(std::net::SocketAddrV4::new(DEFAULT_IP, 9090))
}

fn default_listen_addresses() -> Vec<Multiaddr> {
    vec![format!(
        "/ip4/{}/tcp/{}",
        default_libp2p_api_addr().ip(),
        default_libp2p_api_addr().port()
    )
    .parse()
    .expect(
        r#"
        Listen multiaddresses generation failure.
        This is a critical bug that need to be report on `https://github.com/topos-protocol/topos/issues`
    "#,
    )]
}

fn default_public_addresses() -> Vec<Multiaddr> {
    vec![format!(
        "/ip4/{}/tcp/{}",
        default_libp2p_api_addr().ip(),
        default_libp2p_api_addr().port()
    )
    .parse()
    .expect(
        r#"
        Public multiaddresses generation failure.
        This is a critical bug that need to be report on `https://github.com/topos-protocol/topos/issues`
    "#,
    )]
}
