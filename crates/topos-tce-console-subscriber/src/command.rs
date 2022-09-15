use tokio::sync::mpsc;
use topos_tce_console_api::instrument::WatchUpdatesResponse;

pub enum Command {
    Instrument(Watch),
}

pub struct Watch(pub mpsc::Sender<Result<WatchUpdatesResponse, tonic::Status>>);

impl Watch {
    pub fn update(&self, update: &WatchUpdatesResponse) -> bool {
        if let Ok(reserve) = self.0.try_reserve() {
            reserve.send(Ok(update.clone()));
            true
        } else {
            false
        }
    }
}
