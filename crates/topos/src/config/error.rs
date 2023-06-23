use serde::{Deserialize, Serialize};

///TODO: Remove `#[serde(skip]` and implement Deserialize for Io and Toml errors
#[derive(Debug, Deserialize)]
#[allow(dead_code)]
pub enum ConfigError {
    MissingField(String),
    InvalidType(InvalidType),
    ParseInput(String),
    #[serde(skip)]
    Io(std::io::Error),
    #[serde(skip)]
    Toml(toml::de::Error),
}

#[derive(Debug, Serialize, Deserialize)]
pub struct InvalidType {
    pub expected: String,
    pub actual: String,
}

impl std::error::Error for ConfigError {}

impl std::fmt::Display for ConfigError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ConfigError::MissingField(field) => write!(f, "Missing field: {}", field),
            ConfigError::InvalidType(err) => write!(
                f,
                "Invalid type: expected {}, got {}",
                err.expected, err.actual
            ),
            ConfigError::ParseInput(err) => write!(f, "Figment error: {}", err),
            ConfigError::Io(err) => write!(f, "IO error: {}", err),
            ConfigError::Toml(err) => write!(f, "Toml error: {}", err),
        }
    }
}
