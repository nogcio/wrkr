use anyhow::Result;
use clap::Parser;

use wrkr_tools_compare_perf::cli::Cli;

fn main() -> Result<()> {
    wrkr_tools_compare_perf::app::run(Cli::parse())
}
