use std::{
    fs::File,
    io::{self, Read},
    path::{Path, PathBuf},
    str::FromStr,
    sync::Arc,
    time::Duration,
};

use opentelemetry::global;
use tokio::{
    signal, spawn,
    sync::{mpsc, oneshot, Mutex},
};
use tonic::transport::Channel;
use topos_core::api::grpc::tce::v1::console_service_client::ConsoleServiceClient;
use topos_p2p::PeerId;
use topos_tce::config::{StorageConfiguration, TceConfiguration};
use tower::Service;
use tracing::{debug, error, info, trace};

use crate::options::input_format::{InputFormat, Parser};
use crate::tracing::setup_tracing;

pub(crate) struct PeerList(pub(crate) Option<String>);

impl Parser<PeerList> for InputFormat {
    type Result = Result<Vec<PeerId>, io::Error>;

    fn parse(&self, PeerList(input): PeerList) -> Self::Result {
        let mut input_string = String::new();
        _ = match input {
            Some(path) if Path::new(&path).is_file() => {
                File::open(path)?.read_to_string(&mut input_string)?
            }
            Some(string) => {
                input_string = string;
                0
            }
            None => io::stdin().read_to_string(&mut input_string)?,
        };

        match self {
            InputFormat::Json => Ok(serde_json::from_str::<Vec<PeerId>>(&input_string)?),
            InputFormat::Plain => Ok(input_string
                .trim()
                .split(&[',', '\n'])
                .filter_map(|s| PeerId::from_str(s.trim()).ok())
                .collect()),
        }
    }
}
