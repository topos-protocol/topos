use std::borrow::Cow;

use libp2p::{
    identify::{Event as IdentifyEvent, Info as IdentifyInfo},
    request_response::ProtocolName,
};
use tracing::info;

use crate::{behaviour::transmission::protocol::TransmissionProtocol, Runtime};

use super::EventHandler;

#[async_trait::async_trait]
impl EventHandler<Box<IdentifyEvent>> for Runtime {
    async fn handle(&mut self, event: Box<IdentifyEvent>) {
        if let IdentifyEvent::Received { peer_id, info, .. } = *event {
            let IdentifyInfo {
                protocol_version,
                listen_addrs,
                protocols,
                ..
            } = info;

            if !self.peer_set.contains(&peer_id)
                && protocol_version.as_bytes() == TransmissionProtocol().protocol_name()
                && protocols.iter().any(|p| {
                    self.swarm
                        .behaviour()
                        .discovery
                        .inner
                        .protocol_names()
                        .contains(&Cow::Borrowed(p.as_bytes()))
                })
            {
                self.peer_set.insert(peer_id);
                for addr in listen_addrs {
                    info!(
                        "Adding self-reported address {} from {} to Kademlia DHT.",
                        addr, peer_id
                    );
                    self.swarm
                        .behaviour_mut()
                        .discovery
                        .inner
                        .add_address(&peer_id, addr);
                }
            }
        }
    }
}
