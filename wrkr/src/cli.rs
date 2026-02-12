use clap::{Args, Parser, Subcommand};
use std::net::IpAddr;
use std::path::PathBuf;
use std::time::Duration;

use crate::script_language::{ScriptLanguage, parse_script_language};

fn parse_duration(input: &str) -> Result<Duration, String> {
    let s = input.trim();
    if s.is_empty() {
        return Err("duration cannot be empty (expected e.g. 10s, 250ms, 1m)".to_string());
    }

    let number_end = s
        .char_indices()
        .find(|(_, ch)| !ch.is_ascii_digit())
        .map_or(s.len(), |(idx, _)| idx);

    if number_end == 0 {
        return Err(format!(
            "invalid duration '{s}' (expected e.g. 10s, 250ms, 1m)"
        ));
    }

    let (number_str, unit_str) = s.split_at(number_end);
    let value: u64 = number_str
        .parse()
        .map_err(|_| format!("invalid duration '{s}' (expected e.g. 10s, 250ms, 1m)"))?;

    let unit = unit_str.trim();
    match unit {
        "" | "s" | "sec" | "secs" | "second" | "seconds" => Ok(Duration::from_secs(value)),
        "ms" | "msec" | "msecs" | "millisecond" | "milliseconds" => {
            Ok(Duration::from_millis(value))
        }
        "us" | "µs" | "usec" | "usecs" | "microsecond" | "microseconds" => {
            Ok(Duration::from_micros(value))
        }
        "ns" | "nsec" | "nsecs" | "nanosecond" | "nanoseconds" => Ok(Duration::from_nanos(value)),
        "m" | "min" | "mins" | "minute" | "minutes" => {
            let secs = value
                .checked_mul(60)
                .ok_or_else(|| format!("duration '{s}' is too large"))?;
            Ok(Duration::from_secs(secs))
        }
        "h" | "hr" | "hrs" | "hour" | "hours" => {
            let secs = value
                .checked_mul(60)
                .and_then(|v| v.checked_mul(60))
                .ok_or_else(|| format!("duration '{s}' is too large"))?;
            Ok(Duration::from_secs(secs))
        }
        _ => Err(format!(
            "invalid duration '{s}' (expected e.g. 10s, 250ms, 1m)"
        )),
    }
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
    long_about = "wrkr is a fast, scriptable load testing tool.\n\nModes:\n- URL mode: `wrkr [options] <url>`\n- Scripting mode: `wrkr run <script.lua>`\n\nBy default, environment variables from the current process are visible to scripts; use `--env KEY=VALUE` to add/override values.",
    after_help = "Examples:\n  # URL mode (no script):\n  wrkr -c 50 -d 10s https://example.com/plaintext\n\n  # scripting:\n  wrkr run examples/plaintext.lua\n  wrkr run examples/plaintext.lua --vus 50 --duration 30s\n  wrkr run examples/json_aggregate.lua --iterations 1000 --output json\n  wrkr run examples/plaintext.lua --env BASE_URL=https://example.com\n\nDocs & examples: https://github.com/nogcio/wrkr",
    arg_required_else_help = true
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Option<Command>,

    #[command(flatten)]
    pub cfg: RunConfigArgs,

    #[command(flatten)]
    pub out: RunOutputArgs,

    #[command(flatten)]
    pub wrk: WrkModeArgs,
}

#[derive(Debug, Subcommand)]
pub enum Command {
    /// Run a load test script
    #[command(
        long_about = "Run a test script and execute its entry function with the configured number of virtual users.\n\nCLI flags override values from the script's `options` table."
    )]
    Run(RunArgs),

    /// Scenario utilities (export, etc.)
    Scenario(ScenarioArgs),

    /// Scaffold a scripting workspace for a specific runtime language
    Init(InitArgs),
}

#[derive(Debug, Args)]
pub struct ScenarioArgs {
    #[command(subcommand)]
    pub command: ScenarioCommand,
}

#[derive(Debug, Subcommand)]
pub enum ScenarioCommand {
    /// Export the resolved scenario to YAML (no run is executed)
    Export(ExportScenarioArgs),
}

#[derive(Debug, Args)]
pub struct ExportScenarioArgs {
    /// Path to the script (.lua)
    pub script: PathBuf,

    /// Output YAML file path
    #[arg(long, value_name = "FILE", default_value = "scenario.yaml")]
    pub out: PathBuf,
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

    /// Script runtime language to scaffold (e.g. lua)
    #[arg(long, value_name = "LANG", value_parser = parse_script_language)]
    pub lang: ScriptLanguage,

    /// Script filename to create in the target directory (defaults based on --lang)
    #[arg(long)]
    pub script: Option<String>,
}

#[derive(Debug, Args)]
pub struct RunArgs {
    /// Path to the script (.lua)
    pub script: PathBuf,

    /// Run a specific scenario by name, or provide a YAML file (.yml/.yaml) describing one or
    /// more scenarios to run. When a YAML file is provided, the script's `Options` table is not parsed.
    #[arg(long, value_name = "NAME|PATH.yml")]
    pub scenario: Option<String>,
}

#[derive(Debug, Args, Clone)]
pub struct RunConfigArgs {
    /// Override iterations (otherwise use `Options.iterations` or default=1)
    #[arg(long, global = true)]
    pub iterations: Option<u64>,

    /// Number of virtual users
    #[arg(long, short = 'c', global = true)]
    pub vus: Option<u64>,

    /// Test duration (e.g. 10s, 250ms, 1m)
    #[arg(long, short = 'd', value_parser = parse_duration, global = true)]
    pub duration: Option<Duration>,

    /// Add/override env vars visible to the script (repeatable, KEY=VALUE).
    /// CLI-provided vars override the current process env.
    #[arg(long = "env", value_name = "KEY=VALUE", global = true)]
    pub env: Vec<String>,
}

#[derive(Debug, Args, Clone)]
pub struct RunOutputArgs {
    /// Output format
    #[arg(long, value_enum, default_value_t = OutputFormat::HumanReadable, global = true)]
    pub output: OutputFormat,

    /// Enable the live HTML dashboard server.
    #[arg(
        long,
        env = "WRKR_DASHBOARD",
        default_value_t = false,
        value_parser = clap::builder::BoolishValueParser::new(),
        global = true
    )]
    pub dashboard: bool,

    /// Write a single self-contained offline HTML report after the run completes.
    #[arg(
        long = "dashboard-out",
        env = "WRKR_DASHBOARD_OUT",
        value_name = "FILE",
        global = true
    )]
    pub dashboard_out: Option<PathBuf>,

    /// Dashboard bind address (loopback only). Default: 127.0.0.1
    #[arg(
        long = "dashboard-bind",
        env = "WRKR_DASHBOARD_BIND",
        value_name = "ADDR",
        default_value = "127.0.0.1",
        global = true
    )]
    pub dashboard_bind: IpAddr,

    /// Dashboard port. Default: 0 (ephemeral)
    #[arg(
        long = "dashboard-port",
        env = "WRKR_DASHBOARD_PORT",
        value_name = "PORT",
        default_value_t = 0,
        global = true
    )]
    pub dashboard_port: u16,

    /// Prometheus Pushgateway base URL (enables pushing during the run), e.g. http://127.0.0.1:9091
    #[arg(
        long = "prom-pushgateway-url",
        env = "WRKR_PROM_PUSHGATEWAY_URL",
        value_name = "URL",
        global = true
    )]
    pub prom_pushgateway_url: Option<String>,

    /// Pushgateway job name. Default: wrkr
    #[arg(
        long = "prom-pushgateway-job",
        env = "WRKR_PROM_PUSHGATEWAY_JOB",
        value_name = "JOB",
        default_value = "wrkr",
        global = true
    )]
    pub prom_pushgateway_job: String,

    /// How often to push metrics to Pushgateway during the run. Default: 1s
    #[arg(
        long = "prom-pushgateway-interval",
        env = "WRKR_PROM_PUSHGATEWAY_INTERVAL",
        value_parser = parse_duration,
        value_name = "DURATION",
        default_value = "1s",
        global = true
    )]
    pub prom_pushgateway_interval: Duration,

    /// Pushgateway grouping label (repeatable, KEY=VALUE). Example: --prom-pushgateway-label instance=ci
    #[arg(
        long = "prom-pushgateway-label",
        env = "WRKR_PROM_PUSHGATEWAY_LABEL",
        value_name = "KEY=VALUE",
        global = true
    )]
    pub prom_pushgateway_label: Vec<String>,
}

#[derive(Debug, Args, Clone, Default)]
pub struct WrkModeArgs {
    /// Target URL (URL mode)
    #[arg(value_name = "URL")]
    pub url: Option<String>,

    /// Concurrency hint (used as a fallback for VUs if -c/--vus is not provided)
    #[arg(short = 't', long = "threads")]
    pub threads: Option<u64>,

    /// Script file (`-s`). Interpreted as a `wrkr` script.
    #[arg(short = 's', long = "script", value_name = "FILE")]
    pub script: Option<PathBuf>,

    /// Request header (repeatable). Accepts KEY:VALUE or KEY=VALUE.
    #[arg(short = 'H', long = "header", value_name = "KEY:VALUE")]
    pub header: Vec<String>,

    /// HTTP method (default: GET)
    #[arg(long, short = 'm', default_value = "GET")]
    pub method: String,

    /// Request body (sent as-is)
    #[arg(long, value_name = "BODY")]
    pub body: Option<String>,

    /// Per-request timeout (duration string, e.g. 250ms, 10s)
    #[arg(long, short = 'T', value_name = "DURATION")]
    pub timeout: Option<String>,

    /// Request name tag (recorded as request metric tag `name`)
    #[arg(long, value_name = "NAME")]
    pub name: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_duration_accepts_common_units() {
        assert_eq!(parse_duration("250ms"), Ok(Duration::from_millis(250)));
        assert_eq!(parse_duration("10s"), Ok(Duration::from_secs(10)));
        assert_eq!(parse_duration("1m"), Ok(Duration::from_secs(60)));
        assert_eq!(parse_duration("2h"), Ok(Duration::from_secs(2 * 60 * 60)));
    }

    #[test]
    fn parse_duration_rejects_invalid_values() {
        assert!(parse_duration("").is_err());
        assert!(parse_duration("abc").is_err());
        assert!(parse_duration("10x").is_err());
    }

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
            Some(Command::Run(args)) => {
                assert_eq!(args.script, PathBuf::from("bench.lua"));
                assert_eq!(cli.cfg.iterations, Some(10));
                assert_eq!(cli.cfg.vus, Some(2));
                assert_eq!(cli.cfg.duration, Some(Duration::from_millis(250)));
                assert_eq!(
                    cli.cfg.env,
                    vec!["FOO=bar".to_string(), "EMPTY=".to_string()]
                );
                assert!(matches!(cli.out.output, OutputFormat::HumanReadable));

                assert!(!cli.out.dashboard);
                assert!(cli.out.dashboard_out.is_none());
                assert_eq!(
                    cli.out.dashboard_bind,
                    IpAddr::V4(std::net::Ipv4Addr::new(127, 0, 0, 1))
                );
                assert_eq!(cli.out.dashboard_port, 0);
            }
            Some(Command::Scenario(_)) => panic!("expected run command"),
            Some(Command::Init(_)) => panic!("expected run command"),
            None => panic!("expected run command"),
        }
    }

    #[test]
    fn cli_parses_init_defaults() {
        let parsed = Cli::try_parse_from(["wrkr", "init", "--lang", "lua"]);
        let cli = match parsed {
            Ok(v) => v,
            Err(err) => panic!("failed to parse args: {err}"),
        };

        match cli.command {
            Some(Command::Init(args)) => {
                assert_eq!(args.dir, PathBuf::from("."));
                assert!(!args.force);
                assert!(!args.vscode);
                assert_eq!(args.lang, ScriptLanguage::Lua);
                assert_eq!(args.script, None);
            }
            Some(Command::Scenario(_)) => panic!("expected init command"),
            Some(Command::Run(_)) => panic!("expected init command"),
            None => panic!("expected init command"),
        }
    }
}
