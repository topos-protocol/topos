use std::{str::FromStr, sync::Arc};

use tokio::sync::{mpsc::Sender, oneshot, RwLock};
use tonic::{Request, Response, Status};
use topos_core::api::grpc::tce::v1::{
    console_service_server::ConsoleService, StatusRequest, StatusResponse,
};
use topos_tce_transport::ValidatorId;

use crate::runtime::InternalRuntimeCommand;

pub(crate) struct TceConsoleService {
    pub(crate) command_sender: Sender<InternalRuntimeCommand>,
    pub(crate) status: Arc<RwLock<StatusResponse>>,
}

#[tonic::async_trait]
impl ConsoleService for TceConsoleService {
    async fn status(
        &self,
        _request: Request<StatusRequest>,
    ) -> Result<Response<StatusResponse>, Status> {
        let status = self.status.read().await;

        Ok(Response::new(status.clone()))
    }
}
