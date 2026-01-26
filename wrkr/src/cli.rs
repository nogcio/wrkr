use clap::{Args, Parser, Subcommand};
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

    /// Scaffold a scripting workspace for a specific runtime language
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
        let parsed = Cli::try_parse_from(["wrkr", "init", "--lang", "lua"]);
        let cli = match parsed {
            Ok(v) => v,
            Err(err) => panic!("failed to parse args: {err}"),
        };

        match cli.command {
            Command::Init(args) => {
                assert_eq!(args.dir, PathBuf::from("."));
                assert!(!args.force);
                assert!(!args.vscode);
                assert_eq!(args.lang, ScriptLanguage::Lua);
                assert_eq!(args.script, None);
            }
            Command::Run(_) => panic!("expected init command"),
        }
    }
}
