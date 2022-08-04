//!
//! Here we do simulation of interaction of multiple
//! local protocol instances.
//!

use clap::Parser;
use fastapprox::faster::ln;
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
    ln(n_u as f32) as usize
}

pub fn minimize_params(input: InputConfig) -> Option<SimulationConfig> {
    let n = input.nb_peers as f32;

    // Sample
    let min_sample: usize = 2 * sample_lower_bound(input.nb_peers); // Lower bound
    let max_sample: usize = (n / 4.) as usize; // Upper bound: Quorum size ~ 1/3, in practice hope for 1/4 max
    let sample_candidates = (min_sample..=max_sample).collect::<Vec<_>>();

    // Echo threshold
    let min_echo_threshold_percent: usize = 50; // Percent as usize to use range
    let max_echo_threshold_percent: usize = 66; // Percent as usize to use range
    let echo_threshold_candidates =
        (min_echo_threshold_percent..=max_echo_threshold_percent).collect::<Vec<_>>();
    let echo_ready_candidates = (25..=30).collect::<Vec<_>>();

    let mut best_run: Option<SimulationConfig> = None;
    // let's be linear starting by the fast runs
    for s in sample_candidates {
        for e in &echo_threshold_candidates {
            for r in &echo_ready_candidates {
                if let Some(record) =
                    viable_run(s, (*e as f32) * 0.01, (*r as f32) * 0.01, 0.66, &input)
                {
                    best_run = Some(record);
                    println!("ECHO THRESHOLD : {}", e);
                    break;
                }
            }
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

    let path = "result-ln.csv";

    let mut output = File::create(path)?;
    std::writeln!(output, "N;MS;MS%;E;E%;ET;RT;DT")?;

    let mut network_size = 32;
    for _ in 0..=60 {
        log::info!("Minimizing for N = {:?}", network_size);
        input_config.nb_peers = network_size;
        match minimize_params(input_config.clone()) {
            Some(best_record) => {
                log::info!("🥇 Best Values:\t{:?}", best_record);
                std::writeln!(output, "{}", format_args!("{}", best_record)).ok();
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
        match network_size.cmp(&1024) {
            std::cmp::Ordering::Greater => network_size += 1024,
            std::cmp::Ordering::Less => network_size *= 2,
            std::cmp::Ordering::Equal => network_size += 512,
        }
    }

    Ok(())
}
