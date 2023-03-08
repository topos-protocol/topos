use std::{
    future::Future,
    io::Error,
    pin::Pin,
    task::{Context, Poll},
};

use futures::FutureExt;
use topos_core::api::tce::v1::PushPeerListRequest;
use tower::Service;
use tracing::{debug, error, info, trace};
use uuid::Uuid;

use crate::{
    components::tce::{commands::PushPeerList, PeerList, TCEService},
    options::input_format::Parser,
};

impl Service<PushPeerList> for TCEService {
    type Response = ();

    type Error = std::io::Error;

    type Future =
        Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>> + Send + 'static>>;

    fn poll_ready(&mut self, _cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        Poll::Ready(Ok(()))
    }

    fn call(
        &mut self,
        PushPeerList {
            format,
            peers,
            force,
        }: PushPeerList,
    ) -> Self::Future {
        let client = self.client.clone();

        async move {
            trace!("Building the peers from the input...");
            let peers = format
                .parse(PeerList(peers))?
                .into_iter()
                .map(|p| p.to_string())
                .collect::<Vec<_>>();

            trace!("Peer list built: {:?}", peers);

            if peers.is_empty() && !force {
                error!("Pushing an empty list is prevented unless you provide the --force flag");

                return Err(Error::new(
                    std::io::ErrorKind::Other,
                    "Do not push empty peer list",
                ));
            }

            debug!("Sending the request to the TCE server...");
            if let Err(err) = client
                .lock()
                .await
                .push_peer_list(PushPeerListRequest {
                    request_id: Some(Uuid::new_v4().into()),
                    peers,
                })
                .await
            {
                error!("TCE server returned an error: {:?}", err);
                return Err(Error::new(std::io::ErrorKind::Other, err));
            }

            info!("Successfully pushed the peer list to the TCE");

            Ok(())
        }
        .boxed()
    }
}
