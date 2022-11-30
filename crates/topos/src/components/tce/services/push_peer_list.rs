use std::{
    future::Future,
    pin::Pin,
    task::{Context, Poll},
};

use futures::FutureExt;
use topos_core::api::tce::v1::PushPeerListRequest;
use tower::Service;
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

    fn call(&mut self, PushPeerList { format, peers }: PushPeerList) -> Self::Future {
        let client = self.client.clone();

        async move {
            let peers = format
                .parse(PeerList(peers))?
                .into_iter()
                .map(|p| p.to_string())
                .collect();

            _ = client
                .lock()
                .await
                .push_peer_list(PushPeerListRequest {
                    request_id: Some(Uuid::new_v4().into()),
                    peers,
                })
                .await;
            Ok(())
        }
        .boxed()
    }
}
