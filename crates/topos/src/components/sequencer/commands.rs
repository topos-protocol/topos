use clap::{Args, Subcommand};

mod run;

pub(crate) use run::Run;

/// Topos CLI subcommand used to execute sequencer component
#[derive(Args, Debug)]
pub(crate) struct SequencerCommand {
    #[clap(from_global)]
    pub(crate) verbose: u8,

    #[clap(subcommand)]
    pub(crate) subcommands: Option<SequencerCommands>,
}

#[derive(Subcommand, Debug)]
pub(crate) enum SequencerCommands {
    Run(Box<Run>),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_run() {
        assert!(SequencerCommands::has_subcommand("run"));
    }
}
