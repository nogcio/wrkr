use anyhow::Context as _;
use std::collections::BTreeMap;
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::path::Path;
use std::sync::Arc;

use crate::cli::OutputFormat;
use crate::cli::RunArgs;
use crate::output;
use crate::web::{Dashboard, WebUi, WebUiConfig};

pub async fn run(args: RunArgs) -> anyhow::Result<()> {
    let out = output::formatter(args.output);

    let script = read_script(&args.script).await?;
    let env = merged_env(&args.env)?;
    let cfg = run_config(&args);
    let script_path = Some(args.script.as_path());

    if !args.dashboard
        && args.dashboard_out.is_some()
        && (args.dashboard_port.is_some() || args.dashboard_bind.is_some())
    {
        anyhow::bail!("--dashboard-port/--dashboard-bind requires --dashboard");
    }

    let dashboard_out = args.dashboard_out.clone();

    let dashboard = if args.dashboard || dashboard_out.is_some() {
        Some(Arc::new(Dashboard::new()))
    } else {
        None
    };

    let mut web_ui = if args.dashboard {
        let bind_addr = dashboard_bind_addr(&args)?;
        if !bind_addr.ip().is_loopback() {
            anyhow::bail!(
                "--dashboard-bind must be a loopback address (got {bind_addr}); remote binding is not supported"
            );
        }

        let dashboard = dashboard
            .as_ref()
            .context("dashboard should be initialized when --dashboard is set")?
            .clone();
        let web = WebUi::start(WebUiConfig { bind_addr }, dashboard).await?;
        eprintln!("dashboard={}", web.url());
        Some(web)
    } else {
        None
    };

    let (summary, threshold_violations) = match script_extension(&args.script) {
        "lua" => {
            run_lua_script(
                &args,
                &script,
                script_path,
                &env,
                cfg,
                out.as_ref(),
                dashboard.as_ref(),
            )
            .await?
        }
        ext => anyhow::bail!(
            "unsupported script extension `{ext}` (expected .lua): {}",
            args.script.display()
        ),
    };

    out.print_summary(&summary)?;

    if let Some(d) = &dashboard {
        d.notify_done();
    }

    if let Some(path) = dashboard_out {
        let dashboard = dashboard
            .as_ref()
            .context("dashboard should be initialized when --dashboard-out is set")?;
        let html = dashboard.render_offline_html()?;
        if let Some(parent) = path.parent().filter(|p| !p.as_os_str().is_empty()) {
            tokio::fs::create_dir_all(parent).await.with_context(|| {
                format!(
                    "failed to create dashboard output dir: {}",
                    parent.display()
                )
            })?;
        }
        tokio::fs::write(&path, html)
            .await
            .with_context(|| format!("failed to write dashboard html: {}", path.display()))?;
    }

    if let Some(d) = web_ui.take() {
        d.shutdown().await;
    }

    if summary.checks_failed > 0 || !threshold_violations.is_empty() {
        anyhow::bail!(
            "run failed: checks_failed={}, thresholds_failed={}",
            summary.checks_failed,
            threshold_violations.len()
        );
    }

    Ok(())
}

fn dashboard_bind_addr(args: &RunArgs) -> anyhow::Result<SocketAddr> {
    if let Some(addr) = args.dashboard_bind {
        return Ok(addr);
    }

    let port = args.dashboard_port.unwrap_or(0);
    Ok(SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), port))
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
    dashboard: Option<&Arc<Dashboard>>,
) -> anyhow::Result<(
    wrkr_core::runner::RunSummary,
    Vec<wrkr_core::runner::ThresholdViolation>,
)> {
    let shared = Arc::new(wrkr_core::runner::SharedStore::default());

    let options_client = Arc::new(wrkr_core::HttpClient::default());
    let options_stats = Arc::new(wrkr_core::runner::RunStats::default());

    let opts = wrkr_lua::parse_script_options(
        script,
        script_path,
        env,
        options_client,
        options_stats,
        shared.clone(),
    )
    .context("failed to parse lua options")?;

    let thresholds = opts.thresholds.clone();
    wrkr_lua::run_setup(script, script_path, env, shared.clone()).context("lua Setup failed")?;

    let scenarios =
        wrkr_core::runner::scenarios_from_options(opts, cfg).context("invalid scenario config")?;

    out.print_header(args.script.as_path(), &scenarios);
    let progress = compose_progress(out.progress(), dashboard.map(|d| d.progress_fn()));

    let summary = wrkr_core::runner::run_scenarios(
        script,
        script_path,
        scenarios,
        env.clone(),
        shared.clone(),
        wrkr_lua::run_vu,
        progress,
    )
    .await
    .context("lua run failed")?;

    // Always attempt Teardown/HandleSummary (even if thresholds will fail).
    wrkr_lua::run_teardown(script, script_path, env, shared.clone())
        .context("lua Teardown failed")?;

    run_lua_handle_summary(
        args.output,
        script,
        script_path,
        env,
        &summary,
        shared.clone(),
    )
    .context("lua HandleSummary failed")?;

    let violations = wrkr_core::runner::evaluate_thresholds(&thresholds, &summary.metrics)
        .map_err(|msg| anyhow::anyhow!(msg))?;
    print_threshold_violations(&violations);

    Ok((summary, violations))
}

fn compose_progress(
    out: Option<wrkr_core::runner::ProgressFn>,
    web: Option<wrkr_core::runner::ProgressFn>,
) -> Option<wrkr_core::runner::ProgressFn> {
    match (out, web) {
        (None, None) => None,
        (Some(p), None) | (None, Some(p)) => Some(p),
        (Some(p1), Some(p2)) => Some(Arc::new(move |u| {
            (p1)(u.clone());
            (p2)(u);
        })),
    }
}

fn run_lua_handle_summary(
    output: OutputFormat,
    script: &str,
    script_path: Option<&Path>,
    env: &wrkr_core::runner::EnvVars,
    summary: &wrkr_core::runner::RunSummary,
    shared: Arc<wrkr_core::runner::SharedStore>,
) -> anyhow::Result<()> {
    let outputs = wrkr_lua::run_handle_summary(script, script_path, env, summary, shared)?;

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
