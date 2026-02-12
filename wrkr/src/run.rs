use std::sync::Arc;

use crate::cli::{OutputFormat, RunArgs, RunConfigArgs, RunOutputArgs};
use crate::dashboard::{
    DashboardCollector, DashboardServer, DashboardServerConfig, write_offline_report,
};
use crate::exit_codes::ExitCode;
use crate::output;
use crate::prometheus_push;
use crate::run_error::RunError;
use crate::run_support::{classify_runtime_create_error, classify_runtime_error, merged_env};
use crate::runtime;
use crate::scenario_yaml;
use crate::subscribers::{Subscriber, Subscribers};
use std::net::SocketAddr;

pub async fn run(
    args: RunArgs,
    cfg_args: RunConfigArgs,
    out_args: RunOutputArgs,
) -> Result<ExitCode, RunError> {
    let out = output::formatter(out_args.output);

    let env = merged_env(&cfg_args.env).map_err(RunError::InvalidInput)?;
    let cfg = wrkr_core::RunConfig {
        iterations: cfg_args.iterations,
        vus: cfg_args.vus,
        duration: cfg_args.duration,
    };

    let runtime = runtime::create_runtime(&args.script).map_err(classify_runtime_create_error)?;
    let mut run_ctx = runtime.create_run_context(&env);

    let (opts, scenarios) = match args.scenario.as_deref() {
        None => {
            let opts = runtime
                .parse_script_options(&run_ctx)
                .map_err(|e| classify_runtime_error("failed to parse script options", e))?;

            let scenarios = wrkr_core::scenarios_from_options(opts.clone(), cfg).map_err(|e| {
                RunError::InvalidInput(anyhow::Error::new(e).context("invalid scenario config"))
            })?;

            (opts, scenarios)
        }
        Some(sel) if scenario_yaml::looks_like_yaml_path(sel) => {
            let scenario_path = std::path::PathBuf::from(sel);
            let opts = scenario_yaml::load_script_options_from_yaml(&scenario_path)
                .await
                .map_err(|e| RunError::InvalidInput(e.context("failed to load scenario YAML")))?;

            let scenarios = wrkr_core::scenarios_from_options(opts.clone(), cfg).map_err(|e| {
                RunError::InvalidInput(anyhow::Error::new(e).context("invalid scenario config"))
            })?;

            (opts, scenarios)
        }
        Some(name) => {
            let opts = runtime
                .parse_script_options(&run_ctx)
                .map_err(|e| classify_runtime_error("failed to parse script options", e))?;

            let mut scenarios =
                wrkr_core::scenarios_from_options(opts.clone(), cfg).map_err(|e| {
                    RunError::InvalidInput(anyhow::Error::new(e).context("invalid scenario config"))
                })?;

            scenarios.retain(|s| s.metrics_ctx.scenario() == name);
            if scenarios.is_empty() {
                return Err(RunError::InvalidInput(anyhow::anyhow!(
                    "unknown scenario: {name}"
                )));
            }

            (opts, scenarios)
        }
    };

    run_ctx.thresholds = Arc::from(opts.thresholds.clone().into_boxed_slice());

    runtime
        .run_setup(&run_ctx)
        .map_err(|e| classify_runtime_error("script Setup failed", e))?;

    out.print_header(args.script.as_path(), &scenarios);

    // Dashboard settings are opt-in (default OFF) and are fully resolved by CLI parsing.
    let dashboard_enabled = out_args.dashboard;
    let dashboard_out = out_args.dashboard_out.clone();
    let dashboard_bind = out_args.dashboard_bind;
    let dashboard_port = out_args.dashboard_port;

    let collector =
        (dashboard_enabled || dashboard_out.is_some()).then(|| DashboardCollector::new(&run_ctx));

    let prom_pusher = if let Some(url) = out_args.prom_pushgateway_url.as_deref() {
        let mut cfg = wrkr_metrics::prometheus::pushgateway::PushgatewayConfig::new(
            url.to_string(),
            out_args.prom_pushgateway_job.clone(),
        );
        cfg.grouping_labels =
            prometheus_push::parse_grouping_labels(&out_args.prom_pushgateway_label)
                .map_err(|e| RunError::InvalidInput(anyhow::anyhow!(e)))?;
        // Ensure concurrent runs don't overwrite each other by default.
        cfg.grouping_labels
            .push(("run".to_string(), prometheus_push::default_run_id()));

        Some(prometheus_push::PrometheusPusher::new(
            run_ctx.metrics.clone(),
            prometheus_push::PrometheusPushConfig {
                pushgateway: cfg,
                interval: out_args.prom_pushgateway_interval,
            },
        ))
    } else {
        None
    };

    let mut server: Option<DashboardServer> = None;
    if dashboard_enabled {
        let bind = SocketAddr::new(dashboard_bind, dashboard_port);
        let cfg = DashboardServerConfig { bind };

        let Some(c) = collector.clone() else {
            return Err(RunError::RuntimeError(anyhow::anyhow!(
                "internal error: dashboard enabled without collector"
            )));
        };
        let s = DashboardServer::start(c, cfg)
            .await
            .map_err(|e| RunError::RuntimeError(e.context("failed to start dashboard server")))?;

        eprintln!("dashboard: http://{}", s.addr);
        server = Some(s);
    }

    let mut subs = Subscribers::new();
    subs.push(Subscriber::progress_opt(out.progress()));
    if let Some(c) = collector.clone() {
        let c2 = c.clone();
        subs.push(Subscriber::both(
            c.progress_fn(),
            async move { c2.mark_done() },
        ));
    }
    if let Some(p) = prom_pusher.clone() {
        let p2 = p.clone();
        subs.push(Subscriber::both(p.progress_fn(), async move {
            p2.push_final().await
        }));
    }
    subs.push(Subscriber::finalizer(async move {
        if let Some(s) = server {
            s.shutdown().await;
        }
    }));

    let progress: Option<wrkr_core::ProgressFn> = subs.build_progress_fn();

    let runtime_for_vu = runtime.clone();
    let summary = wrkr_core::run_scenarios(
        scenarios,
        run_ctx.clone(),
        move |ctx| runtime_for_vu.run_vu(ctx),
        progress,
    )
    .await
    .map_err(|e| match e {
        wrkr_core::Error::ThresholdEval(_) => {
            RunError::InvalidInput(anyhow::Error::new(e).context("invalid thresholds"))
        }
        _ => RunError::ScriptError(anyhow::Error::new(e).context("script run failed")),
    })?;

    runtime
        .run_teardown(&run_ctx)
        .map_err(|e| classify_runtime_error("script Teardown failed", e))?;

    subs.finalize_all().await;

    let outputs = runtime
        .run_handle_summary(&run_ctx, &summary)
        .map_err(|e| classify_runtime_error("script HandleSummary failed", e))?;
    if let Some(outputs) = outputs {
        let cwd = std::env::current_dir().map_err(|e| {
            RunError::RuntimeError(
                anyhow::Error::new(e).context("failed to resolve current working directory"),
            )
        })?;
        wrkr_core::write_output_files(&cwd, &outputs.files).map_err(|e| {
            RunError::RuntimeError(
                anyhow::Error::new(e).context("failed to write HandleSummary output files"),
            )
        })?;

        if matches!(out_args.output, OutputFormat::HumanReadable) {
            if let Some(s) = outputs.stdout {
                print!("{s}");
            }
            if let Some(s) = outputs.stderr {
                eprint!("{s}");
            }
        }
    }

    out.print_summary(&summary)
        .map_err(RunError::RuntimeError)?;

    if let (Some(c), Some(path)) = (collector.as_ref(), dashboard_out.as_ref()) {
        write_offline_report(c, path).await.map_err(|e| {
            RunError::RuntimeError(e.context("failed to write dashboard offline report"))
        })?;
    }

    let checks_failed = summary.scenarios.iter().any(|s| s.checks_failed_total > 0);
    let thresholds_failed = !summary.threshold_violations.is_empty();

    Ok(ExitCode::from_quality_gates(
        checks_failed,
        thresholds_failed,
    ))
}
