mod cli;
mod exit_codes;
mod init;
mod output;
mod run;
mod run_error;
mod runtime;
mod script_language;

use clap::Parser;
use mimalloc::MiMalloc;

#[global_allocator]
static GLOBAL: MiMalloc = MiMalloc;

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
        cli::Command::Run(args) => match run::run(args).await {
            Ok(code) => code.as_i32(),
            Err(err) => {
                eprintln!("{err}");
                err.exit_code().as_i32()
            }
        },
        cli::Command::Init(args) => match init::init(args).await {
            Ok(()) => exit_codes::ExitCode::Success.as_i32(),
            Err(err) => {
                eprintln!("{err:#}");
                exit_codes::ExitCode::RuntimeError.as_i32()
            }
        },
    };

    std::process::exit(code);
}
