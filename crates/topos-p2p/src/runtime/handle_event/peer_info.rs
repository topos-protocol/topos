use std::borrow::Cow;

use libp2p::{
    identify::{Event as IdentifyEvent, Info as IdentifyInfo},
    request_response::ProtocolName,
};

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

            if protocol_version.as_bytes() == TransmissionProtocol().protocol_name()
                && protocols.iter().any(|p| {
                    self.swarm
                        .behaviour()
                        .discovery
                        .protocol_names()
                        .contains(&Cow::Borrowed(p.as_bytes()))
                })
            {
                for addr in listen_addrs {
                    self.swarm
                        .behaviour_mut()
                        .transmission
                        .add_address(&peer_id, addr.clone());
                    self.swarm
                        .behaviour_mut()
                        .discovery
                        .add_address(&peer_id, addr);
                }
            }
        }
    }
}
