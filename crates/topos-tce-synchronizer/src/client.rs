use tokio::sync::mpsc;

use crate::SynchronizerCommand;

pub struct SynchronizerClient {
    #[allow(dead_code)]
    pub(crate) shutdown_channel: mpsc::Sender<()>,
    #[allow(dead_code)]
    pub(crate) commands: mpsc::Sender<SynchronizerCommand>,
}
