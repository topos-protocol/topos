use futures::Future;
use tokio::sync::mpsc;
use topos_core::uci::Certificate;

use super::RuntimeCommand;

#[derive(Clone, Debug)]
pub struct RuntimeClient {
    pub(crate) command_sender: mpsc::Sender<RuntimeCommand>,
}

impl RuntimeClient {
    pub fn dispatch_certificate(
        &self,
        certificate: Certificate,
    ) -> impl Future<Output = ()> + 'static + Send {
        let sender = self.command_sender.clone();

        async move {
            _ = sender
                .send(RuntimeCommand::DispatchCertificate { certificate })
                .await;
        }
    }
}
