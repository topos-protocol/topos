use libp2p::{request_response::ResponseChannel, PeerId};
use mockall::mock;
use topos_p2p::{error::CommandExecutionError, NetworkClient, TransmissionResponse};
use topos_tce_gatekeeper::{GatekeeperClient, GatekeeperError};

mock! {
    pub NetworkClient {}

    impl NetworkClient for NetworkClient {
        fn send_request<T: std::fmt::Debug + Into<Vec<u8>> + 'static, R: TryFrom<Vec<u8>> + 'static>(
            &self,
            to: topos_p2p::PeerId,
            data: T,
            retry_policy: topos_p2p::RetryPolicy,
            protocol: &'static str,
        ) -> futures::future::BoxFuture<'static, Result<R, CommandExecutionError>>;

        fn respond_to_request<T: std::fmt::Debug + Into<Vec<u8>> + 'static>(
            &self,
            data: Result<T, ()>,
            channel: ResponseChannel<Result<TransmissionResponse, ()>>,
            protocol: &'static str,
        ) -> futures::future::BoxFuture<'static, Result<(), CommandExecutionError>>;
    }
}

mock! {
    pub GatekeeperClient {}

    #[async_trait::async_trait]
    impl GatekeeperClient for GatekeeperClient {
        async fn get_random_peers(&self, number: usize) -> Result<Vec<PeerId>, GatekeeperError>;
    }
}
