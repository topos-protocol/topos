use tokio::sync::mpsc;

use super::CheckpointsCollectorCommand;

pub struct CheckpointsCollectorClient {
    #[allow(dead_code)]
    pub(crate) shutdown_channel: mpsc::Sender<()>,
    #[allow(dead_code)]
    pub(crate) commands: mpsc::Sender<CheckpointsCollectorCommand>,
}
