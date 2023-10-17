use std::error::Error;
use std::future::IntoFuture;

use libp2p::PeerId;
use tokio::spawn;
use tokio::task::JoinHandle;

use topos_tce_gatekeeper::{Client, GatekeeperError};

pub async fn create_gatekeeper<P: Into<PeerId>>(
    peer_id: P,
    peer_list: Vec<P>,
) -> Result<(Client, JoinHandle<Result<(), GatekeeperError>>), Box<dyn Error>> {
    let (gatekeeper_client, gatekeeper_runtime) = topos_tce_gatekeeper::Gatekeeper::builder()
        .local_peer_id(peer_id.into())
        .peer_list(peer_list.into_iter().map(|p| p.into()).collect())
        .await
        .expect("Can't create the Gatekeeper");

    let gatekeeper_join_handle = spawn(gatekeeper_runtime.into_future());
    Ok((gatekeeper_client, gatekeeper_join_handle))
}
