use crate::{mem_store::TrbMemStore, ReliableBroadcastClient, ReliableBroadcastConfig};
use crate::{DoubleEchoCommand, Errors, SamplerCommand};
/// Mock for the network and broadcast
use rand::Rng;
use rand_distr::Distribution;
use std::collections::{HashMap, HashSet};
use std::fmt::{Debug, Display, Formatter};
use tokio_stream::StreamExt;

use tce_transport::{ReliableBroadcastParams, TrbpEvents};
use tokio::runtime::Runtime;
use tokio::sync::mpsc;
use tokio::task::JoinHandle;
use tokio::time::{self, Duration};
use topos_core::uci::*;

/// Whether to simulate some random network delay
const NETWORK_DELAY_SIMULATION: bool = false;

/// The maximum allowed simulation duration (use larger number for debugging)
static MAX_TEST_DURATION: Duration = Duration::from_secs(60 * 2);
/// Max time that the simulation can be stalled
/// Stall in the sense no messages get exchanged across the nodes
static MAX_STALL_DURATION: Duration = Duration::from_secs(60);

pub type PeersContainer = HashMap<String, ReliableBroadcastClient>;

#[derive(Debug, Default, Clone)]
pub struct InputConfig {
    pub nb_peers: usize,
    pub nb_subnets: usize,
    pub nb_certificates: usize,
}

#[derive(Default, Clone)]
pub struct SimulationConfig {
    pub input: InputConfig,
    pub params: ReliableBroadcastParams,
}

pub fn sample_lower_bound(n_u: usize) -> usize {
    let k: f32 = 2.;
    (n_u as f32).log(k) as usize
}

impl Debug for SimulationConfig {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        let r = |a, b| (a as f32) / (b as f32) * 100.;
        let echo_t_ratio = r(self.params.echo_threshold, self.params.echo_sample_size);
        let delivery_t_ratio = r(
            self.params.delivery_threshold,
            self.params.delivery_sample_size,
        );
        let ratio_sample = r(self.params.echo_sample_size, self.input.nb_peers);
        let min_sample = sample_lower_bound(self.input.nb_peers);
        std::write!(
            f,
            "N={}\t Ω(N)=({}, {}%)\t S=({}, {}%)\t E_t={}%\t R_t={}%\t D_t={}%",
            self.input.nb_peers,
            min_sample,
            r(min_sample, self.input.nb_peers),
            self.params.echo_sample_size,
            ratio_sample,
            echo_t_ratio,
            self.params.ready_threshold,
            delivery_t_ratio
        )
    }
}

impl Display for SimulationConfig {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        let r = |a, b| (a as f32) / (b as f32) * 100.;
        let echo_t_ratio = r(self.params.echo_threshold, self.params.echo_sample_size);
        let delivery_t_ratio = r(
            self.params.delivery_threshold,
            self.params.delivery_sample_size,
        );
        let ratio_sample = r(self.params.echo_sample_size, self.input.nb_peers);
        let min_sample = sample_lower_bound(self.input.nb_peers);
        std::write!(
            f,
            "{};{};{};{};{};{};{};{}",
            self.input.nb_peers,
            min_sample,
            r(min_sample, self.input.nb_peers),
            self.params.echo_sample_size,
            ratio_sample,
            echo_t_ratio,
            self.params.ready_threshold,
            delivery_t_ratio
        )
    }
}

impl SimulationConfig {
    pub fn new(input: InputConfig) -> Self {
        Self {
            input,
            params: ReliableBroadcastParams::default(),
        }
    }

    pub fn set_sample_size(&mut self, s: usize) {
        self.params.echo_sample_size = s;
        self.params.ready_sample_size = s;
        self.params.delivery_sample_size = s;
    }

    pub fn basic_threshold(&mut self) {
        self.set_threshold(0.66, 0.33, 0.66);
    }

    pub fn set_threshold(&mut self, e_ratio: f32, r_ratio: f32, d_ratio: f32) {
        let g = |a, b| ((a as f32) * b) as usize;
        self.params.echo_threshold = g(self.params.echo_sample_size, e_ratio);
        self.params.ready_threshold = g(self.params.ready_sample_size, r_ratio);
        self.params.delivery_threshold = g(self.params.delivery_sample_size, d_ratio);
    }

    #[allow(dead_code)]
    pub fn default(&mut self) {
        self.set_sample_size(self.input.nb_peers / 4);
        self.basic_threshold();
    }
}

use std::sync::Once;

static INIT: Once = Once::new();

pub fn initialize_tracing() {
    INIT.call_once(|| {
        let agent_endpoint = "127.0.0.1:6831".to_string();
        tce_telemetry::init_tracer(&agent_endpoint, "local-integration-test");
    });
}

pub fn viable_run(
    sample_size: usize,
    echo_ratio: f32,
    ready_ratio: f32,
    deliver_ratio: f32,
    input: &InputConfig,
) -> Option<SimulationConfig> {
    let mut config = SimulationConfig {
        input: input.clone(),
        params: ReliableBroadcastParams::default(),
    };
    config.set_sample_size(sample_size);
    config.set_threshold(echo_ratio, ready_ratio, deliver_ratio);

    let rt = Runtime::new().unwrap();
    let current_config = config.clone();
    let res = rt.block_on(async {
        initialize_tracing();
        run_tce_network(current_config).await
    });

    match res {
        Ok(()) => Some(config),
        Err(_) => None,
    }
}

fn generate_cert(
    subnets: &Vec<SubnetId>,
    nb_cert: usize,
    conflict_ratio: f32,
) -> HashMap<SubnetId, HashMap<CertificateId, Vec<Certificate>>> {
    let mut nonce_state: HashMap<SubnetId, CertificateId> = HashMap::new();
    let mut history_state: HashMap<SubnetId, HashMap<CertificateId, Vec<Certificate>>> =
        HashMap::new();

    // Initialize the genesis of all subnets
    for subnet in subnets {
        nonce_state.insert(subnet.clone(), 0.to_string());
        history_state.insert(subnet.clone(), HashMap::new());
    }

    let mut gen_cert = |is_conflicting| -> (SubnetId, Certificate) {
        let mut rng = rand::thread_rng();
        let selected_subnet = subnets[rng.gen_range(0..subnets.len())].clone();
        let last_cert_id = nonce_state.get_mut(&selected_subnet).unwrap();

        let gen_cert: Certificate;
        if is_conflicting {
            gen_cert = Certificate::new(0.to_string(), selected_subnet.clone(), Default::default());
        } else {
            gen_cert = Certificate::new(
                last_cert_id.clone(),
                selected_subnet.clone(),
                Default::default(),
            );
            *last_cert_id = gen_cert.cert_id.clone();
        }

        (selected_subnet, gen_cert)
    };
    let nb_conflict = (conflict_ratio * nb_cert as f32) as usize;
    for _ in 0..nb_conflict {
        let is_conflicting = true;
        let (current_subnet_id, current_cert) = gen_cert(is_conflicting);
        if let Some(subnet_history) = history_state.get_mut(&current_subnet_id) {
            subnet_history
                .entry(current_cert.prev_cert_id.clone())
                .and_modify(|v| v.push(current_cert.clone()))
                .or_insert_with(|| vec![current_cert]);
        }
    }

    for _ in 0..(nb_cert - nb_conflict) {
        let is_conflicting = false;
        let (current_subnet_id, current_cert) = gen_cert(is_conflicting);
        if let Some(subnet_history) = history_state.get_mut(&current_subnet_id) {
            subnet_history
                .entry(current_cert.prev_cert_id.clone())
                .and_modify(|v| v.push(current_cert.clone()))
                .or_insert_with(|| vec![current_cert]);
        }
    }
    history_state
}

#[test]
fn test_cert_conflict_generation() {
    let nb_subnet = 3;
    let nb_cert = 50;
    let conflict_ratio = 0.3;
    let all_subnets: Vec<SubnetId> = (1..=nb_subnet as u64).map(|v| v.to_string()).collect();
    let history_state = generate_cert(&all_subnets, nb_cert, conflict_ratio);
    let mut conflict = false;
    for (_, history_of_subnet) in history_state {
        conflict = history_of_subnet.values().any(|v| v.len() > 1) || conflict;
    }
    assert!(conflict, r#"No conflicting certificates were found!"#);
}

fn submit_test_cert(
    certificates: Vec<Certificate>,
    peers_container: PeersContainer,
    to_peer: String,
) {
    for cert in certificates {
        let mb_cli = peers_container.get(&*to_peer);
        if let Some(w_cli) = mb_cli {
            let sender = w_cli.get_double_echo_channel();
            tokio::spawn(async move {
                sender
                    .send(DoubleEchoCommand::Broadcast { cert: cert.clone() })
                    .await
                    .unwrap();
            });
        };
    }
}

async fn run_tce_network(simu_config: SimulationConfig) -> Result<(), ()> {
    log::info!("{:?}", simu_config);

    let conflict_ratio = 0.;
    let all_peer_ids: Vec<String> = (1..=simu_config.input.nb_peers)
        .map(|e| format!("peer{}", e))
        .collect();
    let all_subnets: Vec<SubnetId> = (1..=simu_config.input.nb_subnets)
        .map(|id| id.to_string())
        .collect();

    // channel for combined event's from all the instances
    let (tx_combined_events, rx_combined_events) =
        mpsc::unbounded_channel::<(String, TrbpEvents)>();

    let trbp_peers = launch_broadcast_protocol_instances(
        all_peer_ids.clone(),
        tx_combined_events,
        all_subnets.clone(),
        simu_config.params.clone(),
    );
    let (tx_exit, main_jh) = launch_simulation_main_loop(trbp_peers.clone(), rx_combined_events);
    let cert_list = generate_cert(
        &all_subnets,
        simu_config.input.nb_certificates,
        conflict_ratio,
    );
    let nodes_history = cert_list
        .values()
        .collect::<Vec<&HashMap<CertificateId, Vec<Certificate>>>>();
    let mut all_cert = Vec::<Certificate>::new();
    for certs_map in nodes_history.clone() {
        for certs_vec in certs_map.values() {
            for cert in certs_vec.clone() {
                all_cert.push(cert);
            }
        }
    }
    let mut node_history: Vec<Certificate> = Vec::new();
    for history in nodes_history {
        for vec_certs in history.values().collect::<Vec<_>>() {
            if !vec_certs.is_empty() {
                node_history = vec_certs.clone();
            }
        }
    }
    // submit test certificate
    // and check for the certificate propagation
    // have to give the nodes some time to arrange with peers
    time::sleep(Duration::from_secs(30)).await;
    submit_test_cert(all_cert.clone(), trbp_peers.clone(), "peer1".to_string());

    watch_cert_delivered(
        trbp_peers.clone(),
        node_history,
        tx_exit.clone(),
        all_peer_ids.clone(),
    );

    // wait for the test completion
    match main_jh.await {
        Err(_) | Ok(Err(_)) => return Err(()),
        _ => {}
    }

    Ok(())
}

fn watch_cert_delivered(
    peers_container: PeersContainer,
    certs: Vec<Certificate>,
    tx_exit: mpsc::UnboundedSender<Result<(), ()>>,
    to_peers: Vec<String>,
) {
    tokio::spawn(async move {
        let mut remaining_peers_to_finish: HashSet<String> = to_peers.iter().cloned().collect();

        let mut interval = time::interval(Duration::from_secs(4));
        while !remaining_peers_to_finish.is_empty() {
            interval.tick().await;
            for ref peer in remaining_peers_to_finish.clone() {
                let mb_cli = peers_container.get(peer);
                if let Some(w_cli) = mb_cli {
                    let mut delivered_all_cert = true;
                    for cert in &certs {
                        if let Ok(delivered) = w_cli
                            .delivered_certs_ids(
                                cert.initial_subnet_id.clone(),
                                cert.cert_id.clone(),
                            )
                            .await
                        {
                            // if something was returned, we'd expect our certificate to be on the list
                            if !delivered.contains(&cert.cert_id) {
                                delivered_all_cert = false;
                            }
                        }
                    }
                    if delivered_all_cert {
                        remaining_peers_to_finish.remove(&peer.clone());
                    }
                }
            }

            log::trace!("Remaining ones: {}", remaining_peers_to_finish.len());
        }

        // when done call signal to exit
        log::info!("🎉 Totality for all the certificates!");
        let _ = tx_exit.send(Ok(()));
    });
}

type SimulationResponse = (
    mpsc::UnboundedSender<Result<(), ()>>,
    JoinHandle<Result<(), ()>>,
);

/// Runs main test loop
///
/// Returns tuple of
/// * combined events sender (peer_id, events)
/// * exit event sender
/// * join handle of the main loop (to await upon)
fn launch_simulation_main_loop(
    peers_container: PeersContainer,
    mut rx_combined_events: mpsc::UnboundedReceiver<(String, TrbpEvents)>,
) -> SimulationResponse {
    // 'exit' command channel & max test duration
    // do tx_exit.send(()) when the condition is met
    let (tx_exit, mut rx_exit) = mpsc::unbounded_channel::<Result<(), ()>>();
    let max_test_duration = time::sleep(MAX_TEST_DURATION);
    let trbp_peers_2 = peers_container;
    let main_jh = tokio::spawn(async move {
        tokio::pin!(max_test_duration);
        let peers = trbp_peers_2;
        loop {
            tokio::select! {
                val = time::timeout(MAX_STALL_DURATION, rx_combined_events.recv()) => {
                    match val {
                        Ok(Some((from_peer, evt))) => {
                            match evt {
                                TrbpEvents::Die => {
                                    log::error!("The peer {:?} died", from_peer);
                                    return Err(());
                                },
                                _ => {
                                    let peers_cl = peers.clone();
                                    let _ = handle_peer_event(from_peer, evt, peers_cl).await;
                                }
                            }
                        },
                        Ok(None) | Err(_) => {
                            log::error!("The simulation got stalled for {:?}", MAX_STALL_DURATION);
                            return Err(());
                        }
                    }
                }
                // we return from this loop when the test condition is met
                Some(res) = rx_exit.recv() => {
                    return res;
                }
                // ... or timeout happened
                () = &mut max_test_duration => {
                    log::error!("Test took max long duration of {:?}, exiting.", MAX_TEST_DURATION);
                    return Err(());
                }
            }
        }
    });
    (tx_exit, main_jh)
}

/// Initialize protocol instances and build-in them into orchestrated event handling
fn launch_broadcast_protocol_instances(
    peer_ids: Vec<String>,
    tx_combined_events: mpsc::UnboundedSender<(String, TrbpEvents)>,
    all_subnets: Vec<SubnetId>,
    global_trb_params: ReliableBroadcastParams,
) -> PeersContainer {
    let mut peers_container = HashMap::<String, ReliableBroadcastClient>::new();

    // create instances
    for peer in peer_ids {
        let (client, mut event_stream) = ReliableBroadcastClient::new(ReliableBroadcastConfig {
            store: Box::new(TrbMemStore::new(all_subnets.clone())),
            trbp_params: global_trb_params.clone(),
            my_peer_id: peer.clone(),
        });

        let _ = peers_container.insert(peer.clone(), client.clone());

        // configure combined events' listener
        let ev_tx = tx_combined_events.clone();
        let ev_peer = peer.clone();
        let _ = tokio::spawn(async move {
            loop {
                tokio::select! {
                    Some(Ok(evt)) = event_stream.next() => {
                        let _ = ev_tx.send((ev_peer.clone(), evt.clone()));
                    },
                    else => {}
                }
            }
        });
    }

    log::debug!("Network is launched, {:?}", peers_container.len());
    peers_container
}

/// Simulating network delay
fn network_delay() -> time::Sleep {
    let mut rng = rand::thread_rng();
    let dist: rand_distr::Poisson<f64> = rand_distr::Poisson::<f64>::new(4.0).unwrap(); // Specify Poisson lambda to set curve properties
    let sample = dist.sample(&mut rng); // Range should be between 0 and 10 with lambda oof 4.0
    let delta: u64 = rng.gen_range(20..=99);
    let delay = (sample * 50.0) as u64 + delta; // Number of milliseconds of delay in 100 MS increments per Poisson
    log::warn!("Network Delay: {:?}ms", delay);
    time::sleep(Duration::from_millis(delay))
}

/// Allows to tune which peers are 'visible' to other peers.
///
/// For now everybody sees whole simulated net
fn visible_peers_for(peer: String, peers_container: PeersContainer) -> Vec<String> {
    peers_container
        .keys()
        .cloned()
        .filter(|p| *p != peer)
        .collect()
}

/// Simulation of the networking
///
/// For now without delays, timeouts, unavailable peers
/// and similar real-life situations.
pub async fn handle_peer_event(
    from_peer: String,
    evt: TrbpEvents,
    peers_container: PeersContainer,
) -> Result<(), Errors> {
    if NETWORK_DELAY_SIMULATION {
        network_delay().await;
    }
    match evt.to_owned() {
        TrbpEvents::NeedPeers => {
            let visible_peers = visible_peers_for(from_peer.clone(), peers_container.clone());
            let mb_cli = peers_container.get(&*from_peer);
            if let Some(w_cli) = mb_cli {
                let sender = w_cli.get_sampler_channel();
                sender
                    .send(SamplerCommand::PeersChanged {
                        peers: visible_peers,
                    })
                    .await?;
            }
        }
        TrbpEvents::Broadcast { cert } => {
            let mb_cli = peers_container.get(&*from_peer);
            if let Some(w_cli) = mb_cli {
                w_cli
                    .get_double_echo_channel()
                    .send(DoubleEchoCommand::Broadcast { cert })
                    .await?;
            }
        }
        // TrbpEvents::EchoSubscribeReq { peers } => {
        //     for to_peer in peers {
        //         let mb_cli = peers_container.get(&*to_peer);
        //         if let Some(w_cli) = mb_cli {
        //             let cli = w_cli.lock().unwrap();
        //             cli.eval(TrbpCommands::OnEchoSubscribeReq {
        //                 from_peer: from_peer.clone(),
        //             })?;
        //         }
        //     }
        // }
        // TrbpEvents::EchoSubscribeOk { to_peer } => {
        //     let mb_cli = peers_container.get(&*to_peer);
        //     if let Some(w_cli) = mb_cli {
        //         let cli = w_cli.lock().unwrap();
        //         cli.eval(TrbpCommands::OnEchoSubscribeOk { from_peer })?;
        //     }
        // }
        // TrbpEvents::ReadySubscribeReq { peers } => {
        //     for to_peer in peers {
        //         let mb_cli = peers_container.get(&*to_peer);
        //         if let Some(w_cli) = mb_cli {
        //             let cli = w_cli.lock().unwrap();
        //             cli.eval(TrbpCommands::OnReadySubscribeReq {
        //                 from_peer: from_peer.clone(),
        //             })?;
        //         }
        //     }
        // }
        // TrbpEvents::ReadySubscribeOk { to_peer } => {
        //     let mb_cli = peers_container.get(&*to_peer);
        //     if let Some(w_cli) = mb_cli {
        //         let cli = w_cli.lock().unwrap();
        //         cli.eval(TrbpCommands::OnReadySubscribeOk { from_peer })?;
        //     }
        // }
        TrbpEvents::Gossip {
            peers,
            cert,
            digest,
        } => {
            for to_peer in peers {
                let mb_cli = peers_container.get(&*to_peer);
                if let Some(w_cli) = mb_cli {
                    w_cli
                        .get_double_echo_channel()
                        .send(DoubleEchoCommand::Deliver {
                            cert: cert.clone(),
                            digest: digest.clone(),
                        })
                        .await?;
                }
            }
        }
        TrbpEvents::Echo { peers, cert } => {
            for to_peer in peers {
                let mb_cli = peers_container.get(&*to_peer);
                if let Some(w_cli) = mb_cli {
                    w_cli
                        .get_double_echo_channel()
                        .send(DoubleEchoCommand::Echo {
                            from_peer: from_peer.clone(),
                            cert: cert.clone(),
                        })
                        .await?;
                }
            }
        }
        TrbpEvents::Ready { peers, cert } => {
            for to_peer in peers {
                let mb_cli = peers_container.get(&*to_peer);
                if let Some(w_cli) = mb_cli {
                    w_cli
                        .get_double_echo_channel()
                        .send(DoubleEchoCommand::Ready {
                            from_peer: from_peer.clone(),
                            cert: cert.clone(),
                        })
                        .await?;
                }
            }
        }
        evt => {
            log::debug!("[{:?}] Unhandled event: {:?}", from_peer, evt);
        }
    }
    Ok(())
}
