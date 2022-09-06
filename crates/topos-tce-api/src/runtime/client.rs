use tokio::sync::mpsc;

use super::RuntimeCommand;

pub struct RuntimeClient {
    #[allow(dead_code)]
    pub(crate) command_sender: mpsc::Sender<RuntimeCommand>,
}
