use clap::ValueEnum;
use serde::Serialize;

#[derive(ValueEnum, Copy, Clone, Debug, Serialize)]
pub(crate) enum InputFormat {
    Json,
    Plain,
}

pub(crate) trait Parser<T> {
    type Result;

    fn parse(&self, input: T) -> Self::Result;
}
