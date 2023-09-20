use tokio::sync::mpsc;

pub struct CheckpointsCollectorClient {
    #[allow(dead_code)]
    pub(crate) shutdown_channel: mpsc::Sender<()>,
}
