use crate::components::setup::commands::Subnet;
use flate2::read::GzDecoder;
use serde::Deserialize;
use serde_json::Value;
use std::collections::HashSet;
use std::fs::File;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::str::FromStr;
use tar::Archive;
use tokio::{signal, spawn};
use tracing::{error, info};

const GITHUB_REPO_API: &str = "https://api.github.com/repos/";

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("Http client error: {0}")]
    Http(reqwest::Error),
    #[error("Json parsing error: {0}")]
    InvalidJson(serde_json::Error),
    #[error("There are no available release")]
    NoValidRelease,
    #[error("Invalid release metadata")]
    InvalidReleaseMetadata,
    #[error("File io error: {0}")]
    File(std::io::Error),
}

fn map_arch(arch: &str) -> &str {
    match arch {
        "x86" => "x86",
        "x86_64" => "amd64",
        "aarch64" => "arm64",
        _ => "unknown",
    }
}

fn map_os(arch: &str) -> &str {
    match arch {
        "linux" => "linux",
        "macos" => "darwin",
        "windows" => "windows",
        _ => "unknown",
    }
}

/// Calculate expected polygon edge binary name for this platform
/// By convention it is in the format `polygon-edge-<cpu architecture>-<operating system>`
fn determine_binary_release_name(release: &PolygonEdgeRelease) -> String {
    "polygon-edge".to_string()
        + "_"
        + &release.version[1..]
        + "_"
        + map_os(std::env::consts::OS)
        + "_"
        + map_arch(std::env::consts::ARCH)
        + ".tar.gz"
}

/// Download Polygon Edge binary from repository to requested target directory
async fn download_binary(file_name: &str, uri: &str, target_directory: &Path) -> Result<(), Error> {
    info!(
        "Downloading binary `{}` to target directory: {}",
        file_name,
        target_directory.display()
    );

    let response = reqwest::get(uri).await.map_err(Error::Http)?;
    let download_file_path = target_directory.join(Path::new(file_name));
    {
        //Download file
        let mut target_archive_file = match File::create(&download_file_path) {
            Err(e) => {
                error!("Unable to create file: {e}");
                return Err(Error::File(e));
            }
            Ok(file) => file,
        };

        target_archive_file
            .write_all(response.bytes().await.map_err(Error::Http)?.as_ref())
            .map_err(Error::File)?;
    }

    {
        // Decompress archive
        let archive_file = File::open(&download_file_path).map_err(Error::File)?;
        let mut archive = Archive::new(GzDecoder::new(archive_file));
        archive.unpack(target_directory).map_err(Error::File)?;
    }

    // Remove downloaded archive
    std::fs::remove_file(&download_file_path).map_err(Error::File)?;

    Ok(())
}

#[derive(Debug)]
struct PolygonEdgeRelease {
    version: String,
    binary: String,
    download_url: String,
}

async fn get_available_releases(repository: &str) -> Result<Vec<PolygonEdgeRelease>, Error> {
    // Retrieve list of releases
    let uri = GITHUB_REPO_API.to_string() + repository + "/releases";

    info!("Retrieving Polygon Edge release list {uri}");
    let client = reqwest::Client::new();
    let body = client
        .get(&uri)
        .header(reqwest::header::USER_AGENT, "Topos CLI")
        .send()
        .await
        .map_err(Error::Http)?
        .text()
        .await
        .map_err(Error::Http)?;

    let body: Vec<Value> = match serde_json::from_str(&body) {
        Ok(v) => v,
        Err(e) => {
            error!("Error parsing release list response: {e}");
            return Err(Error::InvalidJson(e));
        }
    };

    if body.is_empty() {
        error!("There is no valid Polygon Edge release available");
        return Err(Error::NoValidRelease);
    }

    let mut releases: Vec<PolygonEdgeRelease> = Vec::new();
    // Parse all releases
    // List of retrieved releases is already sorted, latest release being
    // the first one in the list
    for release in &body {
        let tag_name = release
            .get("name")
            .ok_or(Error::InvalidReleaseMetadata)?
            .to_string()
            .replace('\"', "");

        let assets = release
            .get("assets")
            .ok_or(Error::InvalidReleaseMetadata)?
            .as_array()
            .ok_or(Error::InvalidReleaseMetadata)?;
        for asset in assets {
            if let Some(name) = asset.get("name").map(|v| v.to_string().replace('\"', "")) {
                if let Some(url) = asset
                    .get("browser_download_url")
                    .map(|v| v.to_string().replace('\"', ""))
                {
                    releases.push(PolygonEdgeRelease {
                        binary: name,
                        download_url: url,
                        version: tag_name.clone(),
                    })
                }
            }
        }
    }

    Ok(releases)
}

/// Get list of releases from github repository
/// Download required release by version, or latest one if desired release was not provided
async fn get_release(
    repository: &str,
    version: &Option<String>,
) -> Result<PolygonEdgeRelease, Error> {
    let releases = get_available_releases(repository).await?;
    for release in releases {
        let expected_binary = determine_binary_release_name(&release);
        if let Some(version) = version {
            if &release.version == version && release.binary == expected_binary {
                return Ok(release);
            }
        } else if release.binary == expected_binary {
            return Ok(release);
        }
    }

    Err(Error::NoValidRelease)
}

pub async fn install_polygon_edge(cmd: Box<Subnet>) -> Result<(), Error> {
    // Select release for installation
    let release = get_release(cmd.repository.as_str(), &cmd.release).await?;

    info!(
        "Selected release: {} from {}",
        release.version, release.download_url
    );

    // Download and install Polygon Edge binary
    if let Err(e) = download_binary(&release.binary, &release.download_url, &cmd.path).await {
        error!("Unable to install Polygon Edge binary {e}");
        return Err(e);
    }

    Ok(())
}

pub async fn list_polygon_edge_releases(cmd: Box<Subnet>) -> Result<(), Error> {
    // Retrieve list of available releases from the Github repository
    let releases = get_available_releases(&cmd.repository).await?;
    println!("Available Polygon Edge releases:");
    releases
        .into_iter()
        .map(|r| r.version)
        .collect::<HashSet<String>>()
        .iter()
        .for_each(|r| {
            println!("   {}", r);
        });

    Ok(())
}
