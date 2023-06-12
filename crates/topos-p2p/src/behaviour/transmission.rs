use crate::error::CommandExecutionError;

use self::{codec::TransmissionCodec, protocol::TransmissionProtocol};

use libp2p::request_response::{Behaviour, Config, ProtocolSupport, RequestId};
use std::{collections::HashMap, iter, time::Duration};
use tokio::sync::oneshot;

pub mod codec;
pub mod protocol;

pub type PendingRequests =
    HashMap<RequestId, oneshot::Sender<Result<Vec<u8>, CommandExecutionError>>>;

/// Transmission is responsible of dealing with node interaction (RequestResponse, Gossip)
pub(crate) struct TransmissionBehaviour {}

impl TransmissionBehaviour {
    pub fn create() -> Behaviour<TransmissionCodec> {
        let mut cfg = Config::default();
        cfg.set_connection_keep_alive(Duration::from_secs(60));
        cfg.set_request_timeout(Duration::from_secs(30));

        Behaviour::new(
            TransmissionCodec(),
            iter::once((TransmissionProtocol(), ProtocolSupport::Full)),
            cfg,
        )
    }
}
