use libp2p::identify::{Event as IdentifyEvent, Info as IdentifyInfo};
use tracing::info;

use crate::{constants::PEER_INFO_PROTOCOL, Runtime};

use super::{EventHandler, EventResult};

#[async_trait::async_trait]
impl EventHandler<Box<IdentifyEvent>> for Runtime {
    async fn handle(&mut self, event: Box<IdentifyEvent>) -> EventResult {
        if let IdentifyEvent::Received { peer_id, info, .. } = *event {
            let IdentifyInfo {
                protocol_version,
                listen_addrs,
                protocols,
                observed_addr,
                ..
            } = info;

            if !self.peer_set.contains(&peer_id)
                && protocol_version.as_bytes() == PEER_INFO_PROTOCOL.as_bytes()
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

        Ok(())
    }
}
