use clap::{Args, Parser, Subcommand};
use std::path::PathBuf;
use std::time::Duration;

fn parse_duration(s: &str) -> Result<Duration, humantime::DurationError> {
    humantime::parse_duration(s)
}

#[derive(Debug, Clone, Copy, clap::ValueEnum)]
pub enum OutputFormat {
    /// Human-readable summary.
    HumanReadable,
    /// Emit JSON progress lines (NDJSON) to stdout.
    Json,
}

#[derive(Debug, Parser)]
#[command(
    name = "wrkr",
    author,
    version,
    about = "Fast, scriptable load testing tool",
    long_about = "wrkr is a fast, scriptable load testing tool.\n\nA test script defines an `options` table (iterations/vus/duration/scenarios) and an entry function to execute per virtual user.\n\nThe current script runtime is Lua and built-in APIs are available via `require(\"wrkr/...\")`.\n\nBy default, environment variables from the current process are visible to the script; use `--env KEY=VALUE` to add/override values.",
    after_help = "Examples:\n  wrkr run examples/plaintext.lua\n  wrkr run examples/plaintext.lua --vus 50 --duration 30s\n  wrkr run examples/json_aggregate.lua --iterations 1000 --output json\n  wrkr run examples/plaintext.lua --env BASE_URL=https://example.com\n\nDocs & examples: https://github.com/nogcio/wrkr"
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Command,
}

#[derive(Debug, Subcommand)]
pub enum Command {
    /// Run a load test script
    #[command(
        long_about = "Run a test script and execute its entry function with the configured number of virtual users.\n\nCLI flags override values from the script's `options` table."
    )]
    Run(RunArgs),

    /// Scaffold a Lua scripting workspace (LuaLS stubs, .luarc.json, and an example script)
    Init(InitArgs),
}

#[derive(Debug, Args)]
pub struct InitArgs {
    /// Target directory to initialize (created if missing)
    #[arg(default_value = ".")]
    pub dir: PathBuf,

    /// Overwrite existing files
    #[arg(long)]
    pub force: bool,

    /// Create VS Code recommendations under .vscode/
    #[arg(long)]
    pub vscode: bool,

    /// Script filename to create in the target directory
    #[arg(long, default_value = "script.lua")]
    pub script: String,
}

#[derive(Debug, Args)]
pub struct RunArgs {
    /// Path to the script (.lua)
    pub script: PathBuf,

    /// Override iterations (otherwise use `options.iterations` or default=1)
    #[arg(long)]
    pub iterations: Option<u64>,

    /// Number of virtual users
    #[arg(long)]
    pub vus: Option<u64>,

    /// Test duration (e.g. 10s, 250ms, 1m)
    #[arg(long, value_parser = parse_duration)]
    pub duration: Option<Duration>,

    /// Add/override env vars visible to the script (repeatable, KEY=VALUE).
    /// CLI-provided vars override the current process env.
    #[arg(long = "env", value_name = "KEY=VALUE")]
    pub env: Vec<String>,

    /// Output format
    #[arg(long, value_enum, default_value_t = OutputFormat::HumanReadable)]
    pub output: OutputFormat,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cli_parses_run_with_iterations() {
        let parsed = Cli::try_parse_from([
            "wrkr",
            "run",
            "bench.lua",
            "--iterations",
            "10",
            "--vus",
            "2",
            "--duration",
            "250ms",
            "--env",
            "FOO=bar",
            "--env",
            "EMPTY=",
            "--output",
            "human-readable",
        ]);

        let cli = match parsed {
            Ok(v) => v,
            Err(err) => panic!("failed to parse args: {err}"),
        };

        match cli.command {
            Command::Run(args) => {
                assert_eq!(args.script, PathBuf::from("bench.lua"));
                assert_eq!(args.iterations, Some(10));
                assert_eq!(args.vus, Some(2));
                assert_eq!(args.duration, Some(Duration::from_millis(250)));
                assert_eq!(args.env, vec!["FOO=bar".to_string(), "EMPTY=".to_string()]);
                assert!(matches!(args.output, OutputFormat::HumanReadable));
            }
            Command::Init(_) => panic!("expected run command"),
        }
    }

    #[test]
    fn cli_parses_init_defaults() {
        let parsed = Cli::try_parse_from(["wrkr", "init"]);
        let cli = match parsed {
            Ok(v) => v,
            Err(err) => panic!("failed to parse args: {err}"),
        };

        match cli.command {
            Command::Init(args) => {
                assert_eq!(args.dir, PathBuf::from("."));
                assert!(!args.force);
                assert!(!args.vscode);
                assert_eq!(args.script, "script.lua".to_string());
            }
            Command::Run(_) => panic!("expected init command"),
        }
    }
}
