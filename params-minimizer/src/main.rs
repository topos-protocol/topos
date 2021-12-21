//!
//! Here we do simulation of interaction of multiple
//! local protocol instances.
//!

use clap::Parser;
use std::fmt::Debug;
use std::fs::File;
use std::io::{Error, Write};
use std::sync::Once;
use tce_trbp::mock::*;

static DB_TEST_SETUP: Once = Once::new();

pub fn setup() {
    DB_TEST_SETUP.call_once(|| {
        // initializing logger
        pretty_env_logger::init_timed();
    });
}

pub fn sample_lower_bound(n_u: usize) -> usize {
    let k: f32 = 2.;
    (n_u as f32).log(k) as usize
}

pub fn minimize_params(input: InputConfig) -> Option<SimulationConfig> {
    let n = input.nb_peers as f32;
    let min_sample: usize = 2 * sample_lower_bound(input.nb_peers); // Lower bound
    let max_sample: usize = (n / 4.) as usize; // Upper bound: Quorum size ~ 1/3, in practice hope for 1/4 max
    let sample_candidates = (min_sample..=max_sample).collect::<Vec<_>>();

    // binary search but involves heavy run
    // let best_sample_size = sample_candidates.partition_point(|s| viable_run(*s, input.clone()));

    let mut best_run: Option<SimulationConfig> = None;
    // let's be linear starting by the fast runs
    for s in sample_candidates {
        if let Some(record) = viable_run(s, &input) {
            best_run = Some(record);
            break;
        }
    }

    best_run
}

/// Application configuration
#[derive(Debug, Parser)]
#[clap(name = "TCE node (toposware.com)")]
pub struct AppArgs {
    /// Number of nodes in the network
    #[clap(long, default_value_t = 1000)]
    pub network_size: usize,
    /// Sample size, same for all of them
    #[clap(long, default_value_t = 300)]
    pub sample_size: usize,
    /// Echo threshold in percent
    #[clap(long, default_value_t = 0.3)]
    pub echo_threshold: f32,
    /// Ready threshold in percent
    #[clap(long, default_value_t = 0.3)]
    pub ready_threshold: f32,
    /// How many certificates to process
    #[clap(long, default_value_t = 1)]
    pub nb_certificates: usize,
    /// How many subnets
    #[clap(long, default_value_t = 1)]
    pub nb_subnets: usize,
}

pub fn main() -> Result<(), Error> {
    setup();
    log::info!("Starting minimizer");

    let args = AppArgs::parse();

    let mut input_config = InputConfig {
        nb_peers: args.network_size,
        nb_subnets: args.nb_subnets,
        nb_certificates: args.nb_certificates,
    };

    let path = "result.csv";

    let mut output = File::create(path)?;
    std::write!(output, "N;S;mean(ms);deviation(ms);E;R;D")?;

    let mut network_size = 128;
    for _ in 0..=10 {
        log::info!("Minimizing for N = {:?}", network_size);
        input_config.nb_peers = network_size;
        match minimize_params(input_config.clone()) {
            Some(best_record) => {
                log::info!("🥇 Best Values:\t{:?}", best_record);
                // std::writeln!(
                //     output,
                //     "{};{};{};{};{};{};{}",
                //     best_record.config.input.nb_peers,
                //     best_record.config.params.echo_sample_size,
                //     best_record.bench.mean,
                //     best_record.bench.deviation,
                //     best_record.config.params.echo_threshold,
                //     best_record.config.params.ready_threshold,
                //     best_record.config.params.delivery_threshold,
                // )?;
            }
            None => {
                log::error!("Failure");
            }
        }
        network_size *= 2;
    }

    Ok(())
}
