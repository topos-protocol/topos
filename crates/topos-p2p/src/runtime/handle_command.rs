use crate::{
    behaviour::transmission::codec::{TransmissionRequest, TransmissionResponse},
    error::FSMError,
    Command, Runtime,
};
use libp2p::PeerId;

impl Runtime {
    pub(crate) async fn handle_command(&mut self, command: Command) {
        match command {
            Command::StartListening { peer_addr, sender } => {
                let _ = sender.send(self.start_listening(peer_addr));
            }

            Command::ConnectedPeers { sender } => {
                let _ = sender.send(Ok(self
                    .swarm
                    .connected_peers()
                    .cloned()
                    .collect::<Vec<_>>()));
            }

            Command::Dial {
                peer_id,
                peer_addr,
                sender,
            } if peer_id != *self.swarm.local_peer_id() => self
                .swarm
                .behaviour_mut()
                .discovery
                .dial(peer_id, peer_addr, sender),

            Command::Dial { sender, .. } => {
                let _ = sender.send(Err(Box::new(FSMError::CantDialSelf)));
            }

            Command::Disconnect { sender } if self.swarm.listeners().count() == 0 => {
                let _ = sender.send(Err(Box::new(FSMError::AlreadyDisconnected)));
            }

            Command::Disconnect { sender } => {
                // TODO: Listeners must be handled by topos behaviour not discovery
                let listeners = self
                    .swarm
                    .behaviour()
                    .discovery
                    .active_listeners
                    .iter()
                    .cloned()
                    .collect::<Vec<_>>();

                listeners.iter().for_each(|listener| {
                    self.swarm.remove_listener(*listener);
                });

                let peers: Vec<PeerId> =
                    self.swarm.connected_peers().into_iter().cloned().collect();

                for peer_id in peers {
                    let _ = self.swarm.disconnect_peer_id(peer_id);
                }

                let _ = sender.send(Ok(()));
            }
            Command::TransmissionReq { to, data, sender } => {
                if self
                    .swarm
                    .behaviour_mut()
                    .transmission
                    .send_request(&to, TransmissionRequest(data), sender)
                    .is_err()
                {
                    // TODO: notify failure
                }
            }

            Command::TransmissionResponse { data, channel } => {
                self.swarm
                    .behaviour_mut()
                    .transmission
                    .send_response(channel, TransmissionResponse(data))
                    .expect("Connection to peer to be still open.");
            }
        }
    }
}
