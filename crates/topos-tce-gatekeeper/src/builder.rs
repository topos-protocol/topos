use std::future::IntoFuture;

use futures::{future::BoxFuture, FutureExt};
use tokio::sync::mpsc;
use topos_p2p::ValidatorId;

use crate::{client::Client, Gatekeeper, GatekeeperError};

#[derive(Default)]
pub struct GatekeeperBuilder {
    local_validator_id: Option<ValidatorId>,
}

impl GatekeeperBuilder {
    pub fn local_validator_id(mut self, validator_id: ValidatorId) -> Self {
        self.local_validator_id = Some(validator_id);

        self
    }
}

impl IntoFuture for GatekeeperBuilder {
    type Output = Result<(Client, Gatekeeper), GatekeeperError>;

    type IntoFuture = BoxFuture<'static, Self::Output>;

    fn into_future(mut self) -> Self::IntoFuture {
        let (shutdown_channel, shutdown) = mpsc::channel(1);
        let (commands, commands_recv) = mpsc::channel(100);

        // TODO: Fix this unwrap later
        let local_validator_id = self.local_validator_id.take().unwrap();

        futures::future::ok((
            Client {
                shutdown_channel,
                commands,
                local_validator_id,
            },
            Gatekeeper {
                shutdown,
                commands: commands_recv,
                ..Gatekeeper::default()
            },
        ))
        .boxed()
    }
}
