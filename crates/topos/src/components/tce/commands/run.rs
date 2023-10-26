use clap::Args;
use serde::Serialize;
use std::collections::HashSet;
use std::net::SocketAddr;
use std::str::FromStr;
use topos_core::types::{ValidatorId, ValidatorIdConversionError};
use topos_p2p::{Multiaddr, PeerId};
use topos_tce_transport::ReliableBroadcastParams;

#[derive(Args, Debug, Serialize)]
#[command(about = "Run a full TCE instance")]
pub struct Run {
    /// Local peer secret key seed (optional, used for testing)
    #[clap(long, env = "TCE_LOCAL_KS")]
    pub local_key_seed: Option<String>,

    /// Local peer secret key seed (optional, used for testing)
    #[clap(long, env = "TCE_LOCAL_VPK")]
    pub local_validator_private_key: Option<String>,

    /// gRPC API Addr
    #[clap(long, env = "TCE_API_ADDR", default_value = "[::1]:1340")]
    pub api_addr: SocketAddr,
}
