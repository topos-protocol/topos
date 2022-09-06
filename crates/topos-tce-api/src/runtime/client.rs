use tokio::sync::mpsc;

use super::RuntimeCommand;

pub struct RuntimeClient {
    pub(crate) command_sender: mpsc::Sender<RuntimeCommand>,
}
