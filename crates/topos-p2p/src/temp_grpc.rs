use tonic::{Response, Status};
use topos_api::grpc::{
    tce::v1::{
        synchronizer_service_client::SynchronizerServiceClient,
        synchronizer_service_server::SynchronizerService, CheckpointRequest, CheckpointResponse,
        FetchCertificatesRequest, FetchCertificatesResponse,
    },
    uci::v1::Certificate,
};
use tracing::info;

pub struct Synchronizer {}

#[async_trait::async_trait]
impl SynchronizerService for Synchronizer {
    async fn fetch_checkpoint(
        &self,
        req: tonic::Request<CheckpointRequest>,
    ) -> Result<tonic::Response<CheckpointResponse>, Status> {
        info!("OK in server");
        Err(Status::unimplemented(""))
    }

    async fn fetch_certificates(
        &self,
        req: tonic::Request<FetchCertificatesRequest>,
    ) -> Result<tonic::Response<FetchCertificatesResponse>, Status> {
        info!("OKKKK In impl");
        let req = req.into_inner();
        let request_id = req.request_id;

        Ok(Response::new(FetchCertificatesResponse {
            request_id,
            certificates: vec![Certificate {
                prev_id: Some([0u8; 32].into()),
                source_subnet_id: Some([1u8; 32].into()),
                state_root: vec![],
                tx_root_hash: vec![],
                receipts_root_hash: vec![],
                target_subnets: vec![[2u8; 32].into()],
                verifier: 0,
                id: Some([3u8; 32].into()),
                proof: None,
                signature: None,
            }],
        }))
    }
}
