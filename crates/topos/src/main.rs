use clap::Parser;

mod components;
mod options;

#[tokio::main]
async fn main() {
    let args = options::Opt::parse();

    match args.commands {
        options::ToposCommand::Tce(cmd) => components::tce::handle_command(cmd).await,
    }
}
