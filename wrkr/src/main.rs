mod cli;
mod init;
mod output;
mod run;

use clap::Parser;
use mimalloc::MiMalloc;

#[global_allocator]
static GLOBAL: MiMalloc = MiMalloc;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cli = cli::Cli::parse();

    match cli.command {
        cli::Command::Run(args) => run::run(args).await,
        cli::Command::Init(args) => init::init(args).await,
    }
}
