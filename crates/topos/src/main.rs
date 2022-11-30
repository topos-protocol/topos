use clap::Parser;
use tokio::{signal, spawn};

mod components;
mod options;

#[tokio::main]
async fn main() {
    let args = options::Opt::parse();

    match args.commands {
        options::ToposCommand::Tce(cmd) => {
            spawn(components::tce::handle_command(cmd));
        }
    }

    signal::ctrl_c()
        .await
        .expect("failed to listen for signals");
}
