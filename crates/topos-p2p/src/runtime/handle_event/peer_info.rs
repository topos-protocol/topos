use ip_network::IpNetwork;
use libp2p::{
    identify::{Event as IdentifyEvent, Info as IdentifyInfo},
    multiaddr::Protocol,
    Multiaddr,
};
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
                    if self.config.allow_private_ip || is_global_addr(&addr) {
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

        Ok(())
    }
}

pub fn is_global_addr(addr: &Multiaddr) -> bool {
    match addr.iter().next() {
        Some(Protocol::Dns(_)) | Some(Protocol::Dns4(_)) | Some(Protocol::Dns6(_)) => true,
        Some(Protocol::Ip4(ip)) => IpNetwork::from(ip).is_global(),
        Some(Protocol::Ip6(ip)) => IpNetwork::from(ip).is_global(),
        _ => false,
    }
}
