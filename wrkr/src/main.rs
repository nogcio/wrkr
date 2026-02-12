mod cli;
mod dashboard;
mod exit_codes;
mod export_scenario;
mod init;
mod output;
mod prometheus_push;
mod run;
mod run_error;
mod run_support;
mod runtime;
mod scenario_yaml;
mod script_language;
mod subscribers;
mod wrk;

use clap::Parser as _;

#[global_allocator]
static GLOBAL: mimalloc::MiMalloc = mimalloc::MiMalloc;

#[tokio::main]
async fn main() {
    let cli = match cli::Cli::try_parse() {
        Ok(v) => v,
        Err(err) => {
            use clap::error::ErrorKind;

            let _ = err.print();
            let code = match err.kind() {
                ErrorKind::DisplayHelp | ErrorKind::DisplayVersion => {
                    exit_codes::ExitCode::Success.as_i32()
                }
                _ => exit_codes::ExitCode::InvalidInput.as_i32(),
            };

            std::process::exit(code);
        }
    };

    let code = match cli.command {
        Some(cli::Command::Run(args)) => match run::run(args, cli.cfg, cli.out).await {
            Ok(code) => code.as_i32(),
            Err(err) => {
                eprintln!("{err}");
                err.exit_code().as_i32()
            }
        },
        Some(cli::Command::Scenario(args)) => match args.command {
            cli::ScenarioCommand::Export(args) => {
                match export_scenario::export_scenario(args, cli.cfg).await {
                    Ok(code) => code.as_i32(),
                    Err(err) => {
                        eprintln!("{err}");
                        err.exit_code().as_i32()
                    }
                }
            }
        },
        Some(cli::Command::Init(args)) => match init::init(args).await {
            Ok(()) => exit_codes::ExitCode::Success.as_i32(),
            Err(err) => {
                eprintln!("{err:#}");
                exit_codes::ExitCode::RuntimeError.as_i32()
            }
        },
        None => match wrk::run_wrk_mode(cli.wrk, cli.cfg, cli.out).await {
            Ok(code) => code.as_i32(),
            Err(err) => {
                eprintln!("{err}");
                err.exit_code().as_i32()
            }
        },
    };

    std::process::exit(code);
}
