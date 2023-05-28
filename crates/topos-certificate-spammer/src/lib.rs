//! Utility to spam dummy certificates

use opentelemetry::trace::FutureExt;
use serde::Deserialize;
use std::collections::HashMap;
use std::fmt::Debug;
use std::sync::Arc;
use tokio::sync::{mpsc, oneshot, Mutex};
use tokio::time::{self, Duration};
use tokio_stream::StreamExt;
use topos_core::uci::*;
use topos_tce_proxy::client::{TceClient, TceClientBuilder};
use tracing::{debug, error, info, info_span, Instrument, Span};
use tracing_opentelemetry::OpenTelemetrySpanExt;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("target nodes are not specified")]
    TargetNodesNotSpecified,
    #[error("error reading target nodes json file:{0}")]
    ErrorReadingTargetNodesJsonFile(String),
    #[error("error parsing target nodes json file:{0}")]
    InvalidTargetNodesJsonFile(String),
    #[error("invalid subnet id error: {0}")]
    InvalidSubnetId(String),
    #[error("hex conversion error {0}")]
    HexConversionError(hex::FromHexError),
    #[error("invalid signing key: {0}")]
    InvalidSigningKey(String),
    #[error("Tce node connection error {0}")]
    TCENodeConnectionError(topos_tce_proxy::Error),
    #[error("Certificate signing error: {0}")]
    CertificateSigningError(topos_core::uci::Error),
}

#[derive(Debug)]
pub struct CertificateSpammerConfig {
    pub target_nodes: Option<Vec<String>>,
    pub target_nodes_path: Option<String>,
    pub local_key_seed: u64,
    pub cert_per_batch: u8,
    pub nb_subnets: u8,
    pub nb_batches: Option<u64>,
    pub batch_interval: u64,
    pub target_subnets: Option<Vec<String>>,
}

fn generate_random_32b_array() -> [u8; 32] {
    (0..32)
        .map(|_| rand::random::<u8>())
        .collect::<Vec<u8>>()
        .try_into()
        .expect("Valid 32 byte array")
}

type NodeApiAddress = String;
#[derive(Deserialize)]
struct FileNodes {
    nodes: Vec<String>,
}

/// Represents connection from one sequencer to a TCE node
/// Multiple different subnets could be connected to the same TCE node address (represented with TargetNodeConnection with different SubnetId and created client)
/// Multiple topos-sequencers from the same subnet could be connected to the same TCE node address (so they would have same SubnetID, but different client instances)
struct TargetNodeConnection {
    address: NodeApiAddress,
    client: Arc<Mutex<TceClient>>,
    shutdown: mpsc::Sender<oneshot::Sender<()>>,
    source_subnet: SourceSubnet,
}

#[derive(Debug, Clone)]
pub struct SourceSubnet {
    signing_key: [u8; 32],
    source_subnet_id: SubnetId,
    last_certificate_id: CertificateId,
}

impl TargetNodeConnection {
    pub async fn shutdown(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        let (sender, receiver) = oneshot::channel();
        self.shutdown.send(sender).await?;
        receiver.await?;

        Ok(())
    }
}

/// Generate test certificate
pub fn generate_test_certificate(
    source_subnet: &mut SourceSubnet,
    target_subnet_ids: &[SubnetId],
) -> Result<Certificate, Box<dyn std::error::Error>> {
    let mut new_cert = Certificate::new(
        source_subnet.last_certificate_id,
        source_subnet.source_subnet_id,
        generate_random_32b_array(),
        generate_random_32b_array(),
        target_subnet_ids,
        0,
        Vec::new(),
    )?;
    new_cert
        .update_signature(&source_subnet.signing_key)
        .map_err(Error::CertificateSigningError)?;

    source_subnet.last_certificate_id = new_cert.id;
    Ok(new_cert)
}

async fn open_target_node_connection(
    nodes: &[String],
    source_subnet: &SourceSubnet,
) -> Result<Vec<TargetNodeConnection>, Error> {
    let mut target_node_connections: Vec<TargetNodeConnection> = Vec::new();
    for tce_address in nodes {
        info!(
            "Opening client for tce service {}, source subnet id: {}",
            &tce_address, &source_subnet.source_subnet_id
        );

        let (tce_client_shutdown_channel, shutdown_receiver) =
            mpsc::channel::<oneshot::Sender<()>>(1);

        let (tce_client, mut receiving_certificate_stream) = match TceClientBuilder::default()
            .set_subnet_id(source_subnet.source_subnet_id)
            .set_tce_endpoint(tce_address)
            .build_and_launch(shutdown_receiver)
            .await
        {
            Ok(value) => value,
            Err(e) => {
                error!(
                    "Unable to create TCE client for node {}, error details: {}",
                    &tce_address, e
                );
                return Err(Error::TCENodeConnectionError(e));
            }
        };

        match tce_client.open_stream(Vec::new()).await {
            Ok(_) => {}
            Err(e) => {
                error!(
                    "Unable to connect to node {}, error details: {}",
                    &tce_address, e
                );
                return Err(Error::TCENodeConnectionError(e));
            }
        }

        let (shutdown_channel, mut shutdown_receiver) = mpsc::channel::<oneshot::Sender<()>>(1);

        let client = Arc::new(Mutex::new(tce_client));
        {
            let source_subnet_id = source_subnet.source_subnet_id;
            let tce_address = tce_address.clone();
            tokio::spawn(async move {
                loop {
                    // process certificates received from the TCE node
                    tokio::select! {
                         Some((cert, position)) = receiving_certificate_stream.next() => {
                            info!("Delivered certificate from tce address: {} for subnet id: {} cert id {}, position {:?}",
                                &tce_address, &source_subnet_id, &cert.id, position);
                         },
                         Some(sender) = shutdown_receiver.recv() => {
                            info!("Shutting down client for tce address: {} for subnet id: {}",
                                &tce_address, &source_subnet_id);

                            let (killer, waiter) = oneshot::channel::<()>();
                            tce_client_shutdown_channel.send(killer).await.unwrap();
                            waiter.await.unwrap();

                            info!("Finishing watch certificates task...");
                            _ = sender.send(());
                            // Finish this task listener
                            break;
                         }
                    }
                }
            });
        }

        target_node_connections.push(TargetNodeConnection {
            address: tce_address.clone(),
            client,
            shutdown: shutdown_channel,
            source_subnet: source_subnet.clone(),
        });
    }
    Ok(target_node_connections)
}

async fn close_target_node_connections(
    target_node_connections: HashMap<SubnetId, Vec<TargetNodeConnection>>,
) {
    for mut target_node in target_node_connections
        .into_iter()
        .flat_map(|(_, connections)| connections)
        .collect::<Vec<TargetNodeConnection>>()
    {
        if let Err(e) = target_node.shutdown().await {
            error!("Error shutting down connection {e}");
        }
    }
}

/// Submit the certificate to the TCE node
fn submit_cert_to_tce(node: &TargetNodeConnection, cert: Certificate) {
    let client = node.client.clone();
    let span = Span::current();
    span.record("certificate_id", cert.id.to_string());
    span.record("source_subnet_id", cert.source_subnet_id.to_string());

    tokio::spawn(
        async move {
            let mut tce_client = client.lock().await;
            send_new_certificate(&mut tce_client, cert)
                .instrument(Span::current())
                .with_current_context()
                .await;
        }
        .with_context(span.context())
        .instrument(span),
    );
}

async fn send_new_certificate(tce_client: &mut TceClient, cert: Certificate) {
    if let Err(e) = tce_client
        .send_certificate(cert)
        .with_current_context()
        .instrument(Span::current())
        .await
    {
        error!(
            "failed to pass certificate to tce client, error details: {}",
            e
        );
    }
}

async fn dispatch(cert: Certificate, target_node: &TargetNodeConnection) {
    info!(
        "Sending cert id={:?} prev_cert_id= {:?} subnet_id={:?} to tce node {}",
        &cert.id, &cert.prev_id, &cert.source_subnet_id, target_node.address
    );
    submit_cert_to_tce(target_node, cert);
}

pub fn generate_source_subnets(
    local_key_seed: u64,
    number_of_subnets: u8,
) -> Result<Vec<SourceSubnet>, Error> {
    let mut subnets = Vec::new();

    let mut signing_key = [0u8; 32];
    let (_, right) = signing_key.split_at_mut(24);
    right.copy_from_slice(local_key_seed.to_be_bytes().as_slice());
    for _ in 0..number_of_subnets {
        signing_key = tiny_keccak::keccak256(&signing_key);

        // Subnet id of the source subnet which will be used for every generated certificate
        let source_subnet_id: SubnetId = topos_crypto::keys::derive_public_key(&signing_key)
            .map_err(|e| Error::InvalidSigningKey(e.to_string()))?
            .as_slice()[1..33]
            .try_into()
            .map_err(|_| Error::InvalidSubnetId("Unable to parse subnet id".to_string()))?;

        subnets.push(SourceSubnet {
            signing_key,
            source_subnet_id,
            last_certificate_id: Default::default(),
        });
    }

    Ok(subnets)
}

pub async fn run(
    args: CertificateSpammerConfig,
    mut shutdown: mpsc::Receiver<oneshot::Sender<()>>,
) -> Result<(), Error> {
    info!(
        "Starting topos certificate spammer with the following arguments: {:#?}",
        args
    );

    // Is list of nodes is specified in the command line use them otherwise use
    // config file provided nodes
    let target_nodes = if let Some(nodes) = args.target_nodes {
        nodes
    } else if let Some(target_nodes_path) = args.target_nodes_path {
        let json_str = std::fs::read_to_string(target_nodes_path)
            .map_err(|e| Error::ErrorReadingTargetNodesJsonFile(e.to_string()))?;

        let json: FileNodes = serde_json::from_str(&json_str)
            .map_err(|e| Error::InvalidTargetNodesJsonFile(e.to_string()))?;
        json.nodes
    } else {
        return Err(Error::TargetNodesNotSpecified);
    };

    // Generate keys for all required subnets (`nb_subnets`)
    let mut source_subnets = generate_source_subnets(args.local_key_seed, args.nb_subnets)?;
    info!("Generated source subnets: {source_subnets:#?}");

    // Target subnets (randomly assigned to every generated certificate)
    let target_subnet_ids: Vec<SubnetId> = args
        .target_subnets
        .iter()
        .flat_map(|id| {
            id.iter().map(|id| {
                let id =
                    hex::decode(&id[2..]).map_err(|e| Error::InvalidSubnetId(e.to_string()))?;
                TryInto::<[u8; 32]>::try_into(id.as_slice())
                    .map_err(|e| Error::InvalidSubnetId(e.to_string()))
            })
        })
        .map(|id| id.map(SubnetId::from_array))
        .collect::<Result<_, _>>()?;

    let mut target_node_connections: HashMap<SubnetId, Vec<TargetNodeConnection>> = HashMap::new();

    // For every source subnet, open connection to every target node, so we will have
    // nb_subnets * len(target_nodes) connections
    for source_subnet in &source_subnets {
        let connections_for_source_subnet =
            open_target_node_connection(target_nodes.as_slice(), source_subnet).await?;
        target_node_connections.insert(
            source_subnet.source_subnet_id,
            connections_for_source_subnet,
        );
    }

    target_node_connections.iter().flat_map(|(_, connections)| connections).for_each(|connection| {
        info!(
            "Certificate spammer target nodes address: {}, source_subnet_id: {}, target subnet ids {:?}",
            connection.address, connection.source_subnet.source_subnet_id, target_subnet_ids
        );
    });

    let number_of_peer_nodes = target_nodes.len();
    let mut batch_interval = time::interval(Duration::from_millis(args.batch_interval));
    let mut batch_number: u64 = 0;

    let shutdown_sender = loop {
        let should_send_batch = tokio::select! {
            _ = batch_interval.tick() => true,
            Some(sender) = shutdown.recv() => {
                info!("Received shutdown signal, stopping certificate spammer");

                for (_, connections) in target_node_connections {
                    for mut connection in connections {
                        info!("Closing connection to target node {}", connection.address);
                        _ = connection.shutdown().await;
                    }
                }


                break Some(sender);
            }
        };

        if should_send_batch {
            // Starting batch, generate cert_per_batch certificates
            batch_number += 1;
            let batch_id = uuid::Uuid::new_v4().to_string();
            // TODO: Need a better name for this span
            let span = info_span!(
                "Batch",
                batch_id,
                batch_number,
                cert_per_batch = args.cert_per_batch,
                number_of_peer_nodes
            );
            async {
                info!("Starting batch {batch_number}");

                let mut batch: Vec<Certificate> = Vec::new(); // Certificates for this batch
                for b in 0..args.cert_per_batch {
                    // Randomize source subnet id
                    let source_subnet =
                        &mut source_subnets[rand::random::<usize>() % args.nb_subnets as usize];
                    // Randomize number of target subnets if target subnet list cli argument is provided
                    let target_subnets: Vec<SubnetId> = if target_subnet_ids.is_empty() {
                        // Empty list of target subnets in certificate
                        Vec::new()
                    } else {
                        // Generate random list in size of 0..len(target_subnet_ids) as target subnets
                        let number_of_target_subnets =
                            rand::random::<usize>() % (target_subnet_ids.len() + 1);
                        let mut target_subnets = Vec::new();
                        for _ in 0..number_of_target_subnets {
                            target_subnets.push(
                                target_subnet_ids
                                    [rand::random::<usize>() % target_subnet_ids.len()],
                            );
                        }
                        target_subnets
                    };

                    let new_cert =
                        match generate_test_certificate(source_subnet, target_subnets.as_slice()) {
                            Ok(cert) => cert,
                            Err(e) => {
                                error!("Unable to generate certificate: {e}");
                                continue;
                            }
                        };
                    debug!("New cert number {b} in batch {batch_number} generated");
                    batch.push(new_cert);
                }

                // Dispatch certs in this batch
                for cert in batch {
                    // Randomly choose target tce node for every certificate from related source_subnet_id connection list
                    let target_node_connection = &target_node_connections[&cert.source_subnet_id]
                        [rand::random::<usize>() % target_nodes.len()];
                    dispatch(cert, target_node_connection)
                        .instrument(Span::current())
                        .with_current_context()
                        .await;
                }
            }
            .instrument(span)
            .await;

            if let Some(nb_batches) = args.nb_batches {
                if batch_number >= nb_batches {
                    info!("Generated {nb_batches}, finishing certificate spammer...");
                    close_target_node_connections(target_node_connections).await;
                    info!("Cert spammer finished");
                    break None;
                }
            }
        }
    };

    info!("Certificate spammer finished");
    if let Some(sender) = shutdown_sender {
        sender
            .send(())
            .expect("Failed to send shutdown signal from certificate spammer");
    }

    Ok(())
}
