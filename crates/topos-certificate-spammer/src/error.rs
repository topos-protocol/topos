#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("target nodes are not specified")]
    TargetNodesNotSpecified,
    #[error("error reading target nodes json file:{0}")]
    ReadingTargetNodesJsonFile(String),
    #[error("error parsing target nodes json file:{0}")]
    InvalidTargetNodesJsonFile(String),
    #[error("invalid subnet id error: {0}")]
    InvalidSubnetId(String),
    #[error("hex conversion error {0}")]
    HexConversion(hex::FromHexError),
    #[error("invalid signing key: {0}")]
    InvalidSigningKey(String),
    #[error("Tce node connection error {0}")]
    TCENodeConnection(topos_tce_proxy::Error),
    #[error("Certificate signing error: {0}")]
    CertificateSigning(topos_core::uci::Error),
}
