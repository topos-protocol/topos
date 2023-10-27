use futures::{future::join_all, StreamExt};
use rand::seq::SliceRandom;
use serde::Deserialize;
use std::time::Duration;
use std::{
    fs::File,
    io::{self, Read},
    path::Path,
};
use tokio::time::error::Elapsed;
use tonic::transport::Uri;
use topos_core::{
    api::grpc::{
        shared::v1::checkpoints::TargetCheckpoint,
        tce::v1::{
            api_service_client::ApiServiceClient,
            console_service_client::ConsoleServiceClient,
            watch_certificates_request::OpenStream,
            watch_certificates_response::{CertificatePushed, Event},
            StatusRequest, SubmitCertificateRequest,
        },
    },
    uci::{Certificate, CERTIFICATE_ID_LENGTH, SUBNET_ID_LENGTH},
};
use tracing::{debug, debug};

use crate::options::input_format::{InputFormat, Parser};

pub(crate) async fn check_delivery(
    timeout_broadcast: u64,
    format: InputFormat,
    peers: Option<String>,
    timeout: u64,
) -> Result<(), Box<dyn std::error::Error>> {
    debug!("peers: {:?}", peers);

    let peers: Vec<Uri> = format
        .parse(NodeList(peers))
        .map_err(|e| format!("Unable to parse node list: {e}"))?
        .into_iter()
        .map(TryInto::try_into)
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| format!("Unable to parse node list: {e}"))?;

    match topos_test_sdk::integration::check_certificate_delivery(timeout_broadcast, peers, timeout)
        .await
    {
        Ok(Err(e)) => Err(format!("Error with certificate delivery: {e:?}").into()),
        Err(e) => Err(Box::new(io::Error::new(
            io::ErrorKind::TimedOut,
            format!("Timeout elapsed: {e}"),
        ))),
        Ok(_) => Ok(()),
    }
}

pub(crate) struct NodeList(pub(crate) Option<String>);

#[derive(Deserialize)]
struct FileNodes {
    nodes: Vec<String>,
}

impl Parser<NodeList> for InputFormat {
    type Result = Result<Vec<String>, io::Error>;

    fn parse(&self, NodeList(input): NodeList) -> Self::Result {
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
            InputFormat::Json => Ok(serde_json::from_str::<FileNodes>(&input_string)?.nodes),
            InputFormat::Plain => Ok(input_string
                .trim()
                .split(&[',', '\n'])
                .map(|s| s.trim().to_string())
                .collect()),
        }
    }
}
