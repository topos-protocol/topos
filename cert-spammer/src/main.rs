//! Utility to spam dummy certificates

use clap::Parser;
use config::{Config, File};
use glob::glob;
use hyper::{header, Body, Client, Method, Request};
use rand::Rng;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fmt::Debug;
use tce_api::web_api::SubmitCertReq;
use tokio::time::{self, Duration};
use topos_core::uci::*;

/// Standalone generator of certificates.
/// This utility aims at spamming the TCE,
/// for test and benchmark purposes
#[derive(Debug, Parser)]
#[clap(name = "Certificate Generator")]
pub struct AppArgs {
    /// The path to the list of target nodes api endpoint (json, toml)
    #[clap(long, env = "TARGET_NODES_PATH")]
    pub target_nodes: String,
    /// The number of certificate emitted per second
    #[clap(long, default_value_t = 100)]
    pub cert_per_sec: usize,
    /// The number of subnets that are emitter
    #[clap(long, default_value_t = 5)]
    pub nb_subnets: usize,
    /// The threshold of byzantine among the subnets between 0. (all correct) and 1. (all byzantine)
    #[clap(long, default_value_t = 0.)]
    pub byzantine_threshold: f32,
    /// The number of nodes to whom we emit the certificate to broadcast
    /// Ideally should work with one node to have the liveness property
    #[clap(long, default_value_t = 1)]
    pub node_per_cert: usize,
}

type NodeApiAddress = String;

#[derive(Debug, Deserialize, Serialize)]
pub struct TargetNodes {
    nodes: Vec<NodeApiAddress>,
}

pub fn gen_cert(
    nb_cert: usize,
    nonce_state: &mut HashMap<SubnetId, CertificateId>,
    subnets: &Vec<SubnetId>,
) -> Vec<Certificate> {
    let mut rng = rand::thread_rng();

    (0..nb_cert)
        .map(|_| {
            let selected_subnet = subnets[rng.gen_range(0..subnets.len())];
            let last_cert_id = nonce_state.get_mut(&selected_subnet).unwrap();
            let gen_cert = Certificate::new(*last_cert_id, selected_subnet, Default::default());
            *last_cert_id = gen_cert.id;
            gen_cert
        })
        .collect()
}

pub fn to_post_request<T: Serialize>(uri: String, input: T) -> Request<Body> {
    let json = serde_json::to_string(&input).expect("serialize");
    Request::builder()
        .method(Method::POST)
        .uri(uri)
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(json))
        .unwrap()
}

/// Submit the certificate to the TCE node
fn submit_cert_to_tce(cert: Certificate, node_api_endpoint: String) {
    let uri = format!("http://{}/certs", node_api_endpoint);
    let cert_req = SubmitCertReq { cert };
    let req = to_post_request(uri, cert_req);
    tokio::spawn(async move {
        let client = Client::new();
        if let Err(e) = client.request(req).await {
            log::error!("Unable to send request: {}", e);
        }
    });
}

pub async fn dispatch(cert_bunch: Vec<Certificate>, nodes: &Vec<NodeApiAddress>) {
    let mut rng = rand::thread_rng();

    for cert in cert_bunch {
        let selected_node = nodes[rng.gen_range(0..nodes.len())].clone();
        log::info!("Sending cert {:?} to {:?}", cert.id, selected_node);
        submit_cert_to_tce(cert, selected_node)
    }
}

#[tokio::main]
async fn main() {
    pretty_env_logger::init_timed();

    let args = AppArgs::parse();
    let settings = Config::builder()
        .add_source(
            glob(&args.target_nodes)
                .unwrap()
                .map(|path| File::from(path.unwrap()))
                .collect::<Vec<_>>(),
        )
        .build()
        .unwrap();

    let target_nodes = settings.try_deserialize::<TargetNodes>().unwrap();
    log::info!("{:?}", target_nodes);

    let subnets: Vec<SubnetId> = (1..=args.nb_subnets as u64).collect();
    let mut nonce_state: HashMap<SubnetId, CertificateId> = HashMap::new();

    // Initialize the genesis of all subnets
    for subnet in &subnets {
        nonce_state.insert(*subnet, 0);
    }

    let mut interval = time::interval(Duration::from_secs(1));
    loop {
        interval.tick().await;
        let cert_bunch = gen_cert(args.cert_per_sec, &mut nonce_state, &subnets);
        dispatch(cert_bunch, &target_nodes.nodes).await;
    }
}
