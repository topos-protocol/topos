use std::{
    future::Future,
    io::Error,
    pin::Pin,
    task::{Context, Poll},
};

use futures::FutureExt;
use topos_core::api::tce::v1::StatusRequest;
use tower::Service;
use tracing::{debug, error};

use crate::components::tce::{commands::Status, TCEService};

impl Service<Status> for TCEService {
    type Response = bool;

    type Error = std::io::Error;

    type Future =
        Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>> + Send + 'static>>;

    fn poll_ready(&mut self, _cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        Poll::Ready(Ok(()))
    }

    fn call(&mut self, _: Status) -> Self::Future {
        let client = self.client.clone();

        async move {
            debug!("Sending the request to the TCE server...");
            match client.lock().await.status(StatusRequest {}).await {
                Ok(status_response) => {
                    debug!("Successfuly fetch the status from the TCE");
                    Ok(status_response.into_inner().has_active_sample)
                }
                Err(err) => {
                    error!("TCE server returned an error: {:?}", err);
                    Err(Error::new(std::io::ErrorKind::Other, err))
                }
            }
        }
        .boxed()
    }
}
