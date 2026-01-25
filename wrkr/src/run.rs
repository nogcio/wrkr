use anyhow::Context as _;
use std::collections::BTreeMap;
use std::path::Path;
use std::sync::Arc;

use crate::cli::OutputFormat;
use crate::cli::RunArgs;
use crate::output;

pub async fn run(args: RunArgs) -> anyhow::Result<()> {
    let out = output::formatter(args.output);

    let script = read_script(&args.script).await?;
    let env = merged_env(&args.env)?;
    let cfg = run_config(&args);
    let script_path = Some(args.script.as_path());

    let summary = match script_extension(&args.script) {
        "lua" => run_lua_script(&args, &script, script_path, &env, cfg, out.as_ref()).await?,
        ext => anyhow::bail!(
            "unsupported script extension `{ext}` (expected .lua): {}",
            args.script.display()
        ),
    };

    out.print_summary(&summary)?;
    Ok(())
}

async fn read_script(path: &Path) -> anyhow::Result<String> {
    tokio::fs::read_to_string(path)
        .await
        .with_context(|| format!("failed to read script: {}", path.display()))
}

fn run_config(args: &RunArgs) -> wrkr_core::runner::RunConfig {
    wrkr_core::runner::RunConfig {
        iterations: args.iterations,
        vus: args.vus,
        duration: args.duration,
    }
}

fn script_extension(path: &Path) -> &str {
    path.extension().and_then(|s| s.to_str()).unwrap_or("")
}

async fn run_lua_script(
    args: &RunArgs,
    script: &str,
    script_path: Option<&Path>,
    env: &wrkr_core::runner::EnvVars,
    cfg: wrkr_core::runner::RunConfig,
    out: &dyn output::OutputFormatter,
) -> anyhow::Result<wrkr_core::runner::RunSummary> {
    let shared = Arc::new(wrkr_core::runner::SharedStore::default());
    let metrics = Arc::new(wrkr_metrics::Registry::default());

    let options_client = Arc::new(wrkr_core::HttpClient::default());

    let opts = wrkr_lua::parse_script_options(
        script,
        script_path,
        env,
        options_client,
        shared.clone(),
        metrics.clone(),
    )
    .context("failed to parse lua options")?;

    wrkr_lua::run_setup(script, script_path, env, shared.clone(), metrics.clone())
        .context("lua Setup failed")?;

    let scenarios =
        wrkr_core::runner::scenarios_from_options(opts, cfg).context("invalid scenario config")?;

    out.print_header(args.script.as_path(), &scenarios);
    let progress = out.progress();

    let summary = wrkr_core::runner::run_scenarios(
        script,
        script_path,
        scenarios,
        env.clone(),
        shared.clone(),
        metrics.clone(),
        wrkr_lua::run_vu,
        progress,
    )
    .await
    .context("lua run failed")?;

    // Always attempt Teardown/HandleSummary (even if thresholds will fail).
    wrkr_lua::run_teardown(script, script_path, env, shared.clone(), metrics.clone())
        .context("lua Teardown failed")?;

    run_lua_handle_summary(
        args.output,
        script,
        script_path,
        env,
        shared.clone(),
        metrics.clone(),
    )
    .context("lua HandleSummary failed")?;

    Ok(summary)
}

fn run_lua_handle_summary(
    output: OutputFormat,
    script: &str,
    script_path: Option<&Path>,
    env: &wrkr_core::runner::EnvVars,
    shared: Arc<wrkr_core::runner::SharedStore>,
    metrics: Arc<wrkr_metrics::Registry>,
) -> anyhow::Result<()> {
    let outputs = wrkr_lua::run_handle_summary(script, script_path, env, shared, metrics)?;

    if let Some(outputs) = &outputs {
        let cwd = std::env::current_dir().context("failed to resolve current working directory")?;
        wrkr_core::runner::write_output_files(&cwd, &outputs.files)
            .context("failed to write HandleSummary output files")?;
    }

    if matches!(output, OutputFormat::HumanReadable)
        && let Some(outputs) = outputs
    {
        if let Some(s) = outputs.stdout {
            print!("{s}");
        }
        if let Some(s) = outputs.stderr {
            eprint!("{s}");
        }
    }

    Ok(())
}

fn print_threshold_violations(violations: &[wrkr_core::runner::ThresholdViolation]) {
    if violations.is_empty() {
        return;
    }

    eprintln!("thresholds_failed: {}", violations.len());
    for v in violations {
        match v.observed {
            Some(o) => eprintln!(
                "threshold_failed: metric={} expr={} observed={o}",
                v.metric, v.expression
            ),
            None => eprintln!(
                "threshold_failed: metric={} expr={} observed=-",
                v.metric, v.expression
            ),
        }
    }
}

fn merged_env(overrides: &[String]) -> anyhow::Result<wrkr_core::runner::EnvVars> {
    let mut map: BTreeMap<String, String> = wrkr_core::runner::process_env_snapshot()
        .iter()
        .map(|(k, v)| (k.to_string(), v.to_string()))
        .collect();

    for raw in overrides {
        let (k, v) = parse_env_override(raw)?;
        map.insert(k, v);
    }

    let vars: Vec<(Arc<str>, Arc<str>)> = map
        .into_iter()
        .map(|(k, v)| (Arc::<str>::from(k), Arc::<str>::from(v)))
        .collect();

    Ok(Arc::from(vars.into_boxed_slice()))
}

fn parse_env_override(s: &str) -> anyhow::Result<(String, String)> {
    let (k, v) = s
        .split_once('=')
        .with_context(|| format!("invalid --env (expected KEY=VALUE): {s}"))?;
    if k.is_empty() {
        anyhow::bail!("invalid --env (empty KEY): {s}");
    }
    Ok((k.to_string(), v.to_string()))
}
