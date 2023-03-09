use crate::sampler::SampleType;

use futures::{Stream, TryStreamExt};
#[allow(unused)]
use opentelemetry::global;
use tokio::sync::mpsc::Sender;
use tokio::sync::{broadcast, mpsc};

use tce_transport::TceEvents;

use topos_core::uci::{Certificate, CertificateId, SubnetId};
use topos_p2p::PeerId;

use crate::{DoubleEchoCommand, Errors, ReliableBroadcastConfig, SamplerCommand};
use tokio_stream::wrappers::BroadcastStream;
pub use topos_core::uci;

#[cfg(feature = "direct")]
#[derive(Clone)]
pub struct ReliableBroadcastClient {
    #[allow(dead_code)]
    event_sender: broadcast::Sender<TceEvents>,

    broadcast_commands: mpsc::Sender<DoubleEchoCommand>,
    sampling_commands: mpsc::Sender<SamplerCommand>,
}

#[cfg(feature = "direct")]
impl ReliableBroadcastClient {
    pub fn new(
        _config: ReliableBroadcastConfig,
        _local_peer_id: String,
    ) -> (Self, impl Stream<Item = Result<TceEvents, ()>>) {
        let (event_sender, event_receiver) = broadcast::channel(2048);
        let (sampler_command_sender, _command_receiver) = mpsc::channel(2048);
        let (broadcast_commands, _command_receiver) = mpsc::channel(2048);

        (
            Self {
                event_sender,
                broadcast_commands,
                sampling_commands: sampler_command_sender,
            },
            BroadcastStream::new(event_receiver).map_err(|_| ()),
        )
    }

    pub async fn peer_changed(&self, _peers: Vec<PeerId>) -> Result<(), ()> {
        Ok(())
    }

    pub async fn force_resample(&self) {}

    pub async fn add_confirmed_peer_to_sample(&self, _sample_type: SampleType, _peer: PeerId) {}

    /// known peers
    /// todo: move it out somewhere out of here, use DHT to advertise urls of API nodes
    pub async fn known_peers_api_addrs(&self) -> Result<Vec<String>, Errors> {
        // todo
        Ok(vec![])
    }

    /// delivered certificates for given target chain after the given certificate
    pub async fn delivered_certs(
        &self,
        _subnet_id: SubnetId,
        _from_cert_id: CertificateId,
    ) -> Result<Vec<Certificate>, Errors> {
        Ok(vec![])
    }

    /// delivered certificates for given target chain after the given certificate
    pub async fn get_span_cert(
        &self,
        _certificate_id: CertificateId,
    ) -> Result<opentelemetry::Context, Errors> {
        unimplemented!()
    }

    pub async fn delivered_certs_ids(
        &self,
        subnet_id: SubnetId,
        from_cert_id: CertificateId,
    ) -> Result<Vec<CertificateId>, Errors> {
        self.delivered_certs(subnet_id, from_cert_id)
            .await
            .map(|mut v| v.iter_mut().map(|c| c.id).collect())
    }

    pub fn get_sampler_channel(&self) -> Sender<SamplerCommand> {
        self.sampling_commands.clone()
    }

    pub fn get_double_echo_channel(&self) -> Sender<DoubleEchoCommand> {
        self.broadcast_commands.clone()
    }

    pub fn get_command_channels(&self) -> (Sender<SamplerCommand>, Sender<DoubleEchoCommand>) {
        (
            self.sampling_commands.clone(),
            self.broadcast_commands.clone(),
        )
    }

    /// Use to broadcast new certificate to the TCE network
    pub async fn broadcast_new_certificate(&self, _certificate: Certificate) -> Result<(), ()> {
        // let broadcast_commands = self.broadcast_commands.clone();
        //
        // info!("send certificate to be broadcast");
        // if broadcast_commands
        //     .send(DoubleEchoCommand::Broadcast {
        //         cert: certificate,
        //         ctx: Span::current().context(),
        //     })
        //     .await
        //     .is_err()
        // {
        //     error!("Unable to send broadcast_new_certificate command, Receiver was dropped");
        // }
        //
        Ok(())
    }

    pub async fn shutdown(&self) -> Result<(), Errors> {
        Ok(())
        // debug!("Shutting down reliable broadcast client");
        // let (double_echo_sender, double_echo_receiver) = oneshot::channel();
        // self.double_echo_shutdown_channel
        //     .send(double_echo_sender)
        //     .await
        //     .map_err(Errors::ShutdownCommunication)?;
        // double_echo_receiver.await?;
        //
        // let (sampler_sender, sampler_receiver) = oneshot::channel();
        // self.sampler_shutdown_channel
        //     .send(sampler_sender)
        //     .await
        //     .map_err(Errors::ShutdownCommunication)?;
        // Ok(sampler_receiver.await?)
    }
}
