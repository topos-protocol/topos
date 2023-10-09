use async_trait::async_trait;
use tonic::Request;
use topos_core::api::grpc::tce::v1::{
    synchronizer_service_server::SynchronizerService, CheckpointRequest, FetchCertificatesRequest,
};

struct MockSynchronizerServer {}

#[async_trait]
impl SynchronizerService for MockSynchronizerServer {
    async fn fetch_checkpoint(
        &self,
        _request: Request<CheckpointRequest>,
    ) -> Result<tonic::Response<topos_core::api::grpc::tce::v1::CheckpointResponse>, tonic::Status>
    {
        todo!()
    }

    async fn fetch_certificates(
        &self,
        _request: Request<FetchCertificatesRequest>,
    ) -> Result<
        tonic::Response<topos_core::api::grpc::tce::v1::FetchCertificatesResponse>,
        tonic::Status,
    > {
        todo!()
    }
}
