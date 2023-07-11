use libp2p::{
    identify::Behaviour as Identify, identify::Config as IdentifyConfig,
    identify::Event as IdentifyEvent, identity::Keypair, swarm::NetworkBehaviour,
};

#[derive(NetworkBehaviour)]
#[behaviour(to_swarm = "IdentifyEvent")]
pub struct PeerInfoBehaviour {
    identify: Identify,
}

impl PeerInfoBehaviour {
    pub(crate) fn new(identify_protocol: &'static str, peer_key: &Keypair) -> PeerInfoBehaviour {
        let ident_config = IdentifyConfig::new(identify_protocol.to_string(), peer_key.public())
            .with_push_listen_addr_updates(true);
        let identify = Identify::new(ident_config);

        Self { identify }
    }
}
