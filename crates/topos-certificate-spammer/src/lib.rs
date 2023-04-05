//! Utility to spam dummy certificates

use rand::Rng;
use serde::Deserialize;
use std::collections::HashMap;
use std::fmt::Debug;
use std::sync::Arc;
use tokio::sync::Mutex;
use tokio::time::{self, Duration};
use tokio_stream::StreamExt;
use topos_core::uci::*;
use topos_sequencer_tce_proxy::{TceClient, TceClientBuilder};
use tracing::{debug, error, info};

#[derive(Debug)]
pub struct CertificateSpammerConfiguration {
    pub target_node: Option<String>,
    pub target_nodes_path: Option<String>,
    pub cert_per_batch: usize,
    pub batch_time_interval: u64,
    pub nb_subnets: usize,
    pub byzantine_threshold: f32,
    pub node_per_cert: usize,
}

type NodeApiAddress = String;

#[derive(Deserialize)]
struct FileNodes {
    nodes: Vec<String>,
}

/// Represents connection from one subnet/topos-node to the TCE node
/// Multiple different subnets could be connected to the same TCE node address (represented with TargetNodeConnection with different SubnetId and created client)
/// Multiple topos-nodes from the same subnet could be connected to the same TCE node address (so they would have same SubnetID, but different client instances)
#[allow(dead_code)]
pub struct TargetNodeConnection {
    address: NodeApiAddress,
    source_subnet_id: SubnetId,
    client: Arc<Mutex<TceClient>>,
}

/// Generate vector of pairs - certificate and matching tce node where this certificate should be sent
pub fn gen_cert<'a>(
    nb_cert: usize,
    nonce_state: &mut HashMap<SubnetId, CertificateId>,
    target_node_connections: &'a Vec<TargetNodeConnection>,
) -> Vec<(&'a TargetNodeConnection, Certificate)> {
    let mut rng = rand::thread_rng();
    debug!("Generating {} certificates...", nb_cert);
    (0..nb_cert)
        .map(|_| {
            let selected_target_node_connection =
                &target_node_connections[rng.gen_range(0..target_node_connections.len())];
            let last_cert_id = nonce_state
                .get_mut(&selected_target_node_connection.source_subnet_id)
                .unwrap();
            let gen_cert = Certificate::new(
                *last_cert_id,
                selected_target_node_connection.source_subnet_id,
                Default::default(),
                [5u8; 32],
                &[[0u8; 32].into(); 1],
                0,
                Vec::new(),
            )
            .expect("Valid certificate");
            *last_cert_id = gen_cert.id;

            (selected_target_node_connection, gen_cert)
        })
        .collect()
}

async fn generate_target_node_connections(
    target_node_addresses: Vec<String>,
    subnets: Vec<SubnetId>,
) -> Result<Vec<TargetNodeConnection>, Box<dyn std::error::Error>> {
    let mut target_nodes: Vec<TargetNodeConnection> = Vec::new();
    for (tce_address, subnet_id) in
        std::iter::zip(target_node_addresses, subnets).collect::<Vec<(String, SubnetId)>>()
    {
        info!(
            "Opening client for tce service {}, subnet id: {:?}",
            &tce_address, &subnet_id
        );

        let (tce_client, mut receiving_certificate_stream) = match TceClientBuilder::default()
            .set_subnet_id(subnet_id)
            .set_tce_endpoint(&tce_address)
            .build_and_launch()
            .await
        {
            Ok(value) => value,
            Err(e) => {
                error!(
                    "Unable to create TCE client for node {}, error details: {}",
                    &tce_address, e
                );
                panic!("Unable to create TCE client");
            }
        };

        match tce_client.open_stream(Vec::new()).await {
            Ok(_) => {}
            Err(e) => {
                error!(
                    "Unable to connect to node {}, error details: {}",
                    &tce_address, e
                );
                panic!("Unable to connect to TCE node");
            }
        }

        {
            let subnet_id = subnet_id;
            let tce_address = tce_address.clone();
            tokio::spawn(async move {
                loop {
                    // process certificates received from the TCE node
                    tokio::select! {
                         Some((cert, position)) = receiving_certificate_stream.next() => {
                            info!("Delivered certificate from tce address: {} for subnet id: {} cert id {}, position {:?}", &tce_address, &subnet_id, &cert.id, position);
                       }
                    }
                }
            });
        }

        target_nodes.push(TargetNodeConnection {
            address: tce_address,
            source_subnet_id: subnet_id,
            client: Arc::new(Mutex::new(tce_client)),
        });
    }
    Ok(target_nodes)
}

/// Submit the certificate to the TCE node
fn submit_cert_to_tce(node: &TargetNodeConnection, cert: Certificate) {
    let client = node.client.clone();
    tokio::spawn(async move {
        let mut client = client.lock().await;
        if let Err(e) = client.send_certificate(cert).await {
            error!(
                "failed to pass certificate to tce client, error details: {}",
                e
            );
        }
    });
}

pub async fn dispatch(cert_bunch: Vec<(&TargetNodeConnection, Certificate)>) {
    for (target_node, cert) in cert_bunch {
        info!(
            "Sending cert id={:?} prev_cert_id= {:?} subnet_id={:?} to tce node {}",
            cert.id, cert.prev_id, cert.source_subnet_id, target_node.address
        );
        submit_cert_to_tce(target_node, cert);
    }
}

pub async fn run(
    config: CertificateSpammerConfiguration,
) -> Result<(), Box<dyn std::error::Error>> {
    info!(
        "Starting certificate spammer with configuration: {:?}",
        config
    );

    let mut target_node_addresses: Vec<String> = Vec::new();
    if let Some(target_node) = config.target_node {
        target_node_addresses.push(target_node);
    }

    if let Some(target_nodes_path) = config.target_nodes_path {
        if target_nodes_path.ends_with(".json") {
            let json_str =
                std::fs::read_to_string(target_nodes_path).expect("Unable to read json file");
            let json: FileNodes =
                serde_json::from_str(&json_str).expect("JSON was not well-formatted");
            for tce_node_address in json.nodes {
                target_node_addresses.push(tce_node_address);
            }
        } else if target_nodes_path.ends_with(".toml") {
            let toml_str =
                std::fs::read_to_string(target_nodes_path).expect("Unable to read toml file");
            let toml_nodes: FileNodes =
                toml::from_str(&toml_str).expect("TOML is not well formatted");
            for toml_node_address in toml_nodes.nodes {
                target_node_addresses.push(toml_node_address);
            }
        } else {
            panic!("Invalid target node list argument, should be `.json` or `.toml` file");
        }
    }

    let subnets: Vec<SubnetId> = (1..=target_node_addresses.len())
        .map(|v| [v as u8; 32].into())
        .collect();

    let target_node_connections =
        generate_target_node_connections(target_node_addresses, subnets).await?;

    for target_node_connection in &target_node_connections {
        info!(
            "Certificate spammer target nodes address: {}, source_subnet_id: {:?}",
            target_node_connection.address, target_node_connection.source_subnet_id
        );
    }

    let mut nonce_state: HashMap<SubnetId, CertificateId> = HashMap::new();

    // Initialize the genesis of all subnets
    info!("Initializing genesis on all subnets...");
    for target_node_connection in &target_node_connections {
        nonce_state.insert(target_node_connection.source_subnet_id, [0u8; 32].into());
    }

    let mut interval = time::interval(Duration::from_secs(config.batch_time_interval));
    loop {
        interval.tick().await;
        let cert_bunch = gen_cert(
            config.cert_per_batch,
            &mut nonce_state,
            &target_node_connections,
        );
        // Dispatch every generated certificate to belonging TCE node (we assume every subnet is connected to one tce node)
        dispatch(cert_bunch).await;
    }
}
