mod perf_gate;

use clap::{Parser, Subcommand};

#[derive(Debug, Parser)]
#[command(name = "xtask")]
#[command(about = "Repo tooling for wrkr (xtask)")]
struct Cli {
    #[command(subcommand)]
    cmd: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// Perf regression gate (runs standard perf suite and checks ratios)
    PerfGate(perf_gate::Args),
}

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    match cli.cmd {
        Command::PerfGate(args) => perf_gate::run(args),
    }
}
