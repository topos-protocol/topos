use console_api::instrument::{
    instrument_client::InstrumentClient, WatchUpdatesRequest, WatchUpdatesResponse,
};
use futures::StreamExt;
use tokio::{spawn, sync::mpsc};

pub async fn open_client() -> mpsc::Receiver<WatchUpdatesResponse> {
    let (sender, receiver) = mpsc::channel(2048);
    spawn(async move {
        let mut client = InstrumentClient::connect("http://localhost:6669")
            .await
            .unwrap();

        let mut stream = client
            .watch_updates(WatchUpdatesRequest {})
            .await
            .unwrap()
            .into_inner();

        while let Some(Ok(update)) = stream.next().await {
            _ = sender.send(update).await;
        }
    });

    receiver
}
