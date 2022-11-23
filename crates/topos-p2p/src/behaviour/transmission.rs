use crate::error::CommandExecutionError;

use self::{codec::TransmissionCodec, protocol::TransmissionProtocol};

use libp2p::request_response::{
    ProtocolSupport, RequestId, RequestResponse, RequestResponseConfig,
};
use std::{collections::HashMap, iter, time::Duration};
use tokio::sync::oneshot;

pub mod codec;
pub mod protocol;

pub type PendingRequests =
    HashMap<RequestId, oneshot::Sender<Result<Vec<u8>, CommandExecutionError>>>;

/// Transmission is responsible of dealing with node interaction (RequestResponse, Gossip)
pub(crate) struct TransmissionBehaviour {}

impl TransmissionBehaviour {
    pub fn create() -> RequestResponse<TransmissionCodec> {
        let mut cfg = RequestResponseConfig::default();
        cfg.set_connection_keep_alive(Duration::from_secs(10));
        cfg.set_request_timeout(Duration::from_secs(30));

        RequestResponse::new(
            TransmissionCodec(),
            iter::once((TransmissionProtocol(), ProtocolSupport::Full)),
            cfg,
        )
    }
}
