use clap::ValueEnum;
use serde::Serialize;

#[derive(ValueEnum, Copy, Clone, Debug, Serialize)]
pub(crate) enum InputFormat {
    Json,
    Plain,
}
