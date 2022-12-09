use tokio::sync::mpsc;

use crate::GatekeeperCommand;

pub struct GatekeeperClient {
    #[allow(dead_code)]
    pub(crate) shutdown_channel: mpsc::Sender<()>,
    #[allow(dead_code)]
    pub(crate) commands: mpsc::Sender<GatekeeperCommand>,
}
