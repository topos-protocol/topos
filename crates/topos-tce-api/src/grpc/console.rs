use std::{str::FromStr, sync::Arc};

use tokio::sync::{mpsc::Sender, oneshot, RwLock};
use tonic::{Request, Response, Status};
use topos_core::api::tce::v1::{
    console_service_server::ConsoleService, PushPeerListRequest, PushPeerListResponse,
    StatusRequest, StatusResponse,
};
use topos_p2p::PeerId;

use crate::runtime::InternalRuntimeCommand;

pub(crate) struct TceConsoleService {
    pub(crate) command_sender: Sender<InternalRuntimeCommand>,
    pub(crate) status: Arc<RwLock<StatusResponse>>,
}

#[tonic::async_trait]
impl ConsoleService for TceConsoleService {
    async fn push_peer_list(
        &self,
        request: Request<PushPeerListRequest>,
    ) -> Result<Response<PushPeerListResponse>, Status> {
        let PushPeerListRequest { peers, .. } = request.into_inner();
        let number_of_peers = peers.len();

        let parsed_peers: Vec<PeerId> = peers
            .into_iter()
            .filter_map(|p| PeerId::from_str(&p).ok())
            .collect();

        if number_of_peers != parsed_peers.len() {
            return Err(Status::invalid_argument("Cannot parse all PeerIds"));
        }

        let (sender, receiver) = oneshot::channel();
        if self
            .command_sender
            .send(InternalRuntimeCommand::PushPeerList {
                peers: parsed_peers,
                sender,
            })
            .await
            .is_err()
        {
            return Err(Status::internal("Can't send command to the Gatekeeper"));
        }

        if receiver.await.is_err() {
            return Err(Status::internal(
                "Gatekeeper refused to update the peer list",
            ));
        }

        Ok(Response::new(PushPeerListResponse {}))
    }

    async fn status(
        &self,
        _request: Request<StatusRequest>,
    ) -> Result<Response<StatusResponse>, Status> {
        let status = self.status.read().await;

        Ok(Response::new(status.clone()))
    }
}
