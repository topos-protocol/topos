use clap::{Args, Subcommand};

mod push_certificate;
mod spam;

pub(crate) use push_certificate::PushCertificate;
pub(crate) use spam::Spam;

/// Run test commands (e.g., pushing a certificate to a TCE process)
#[derive(Args, Debug)]
pub(crate) struct RegtestCommand {
    #[clap(from_global)]
    pub(crate) verbose: u8,

    #[clap(subcommand)]
    pub(crate) subcommands: Option<RegtestCommands>,
}

#[derive(Subcommand, Debug)]
pub(crate) enum RegtestCommands {
    PushCertificate(Box<PushCertificate>),
    Spam(Box<Spam>),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_run() {
        assert!(RegtestCommands::has_subcommand("push-certificate"));
        assert!(RegtestCommands::has_subcommand("spam"));
    }
}
