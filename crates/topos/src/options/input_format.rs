use clap::ValueEnum;

#[derive(ValueEnum, Copy, Clone, Debug)]
pub(crate) enum InputFormat {
    Json,
    Plain,
}

pub(crate) trait Parser<T> {
    type Result;

    fn parse(&self, input: T) -> Self::Result;
}
