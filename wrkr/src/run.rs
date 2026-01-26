use std::collections::BTreeMap;
use std::sync::Arc;

use crate::cli::OutputFormat;
use crate::cli::RunArgs;
use crate::exit_codes::ExitCode;
use crate::output;
use crate::run_error::RunError;
use crate::runtime;
use anyhow::Context as _;

pub async fn run(args: RunArgs) -> Result<ExitCode, RunError> {
    let out = output::formatter(args.output);

    let env = merged_env(&args.env).map_err(RunError::InvalidInput)?;
    let cfg = wrkr_core::RunConfig {
        iterations: args.iterations,
        vus: args.vus,
        duration: args.duration,
    };

    let runtime = runtime::create_runtime(&args.script).map_err(classify_runtime_create_error)?;
    let mut run_ctx = runtime.create_run_context(&env);

    let opts = runtime
        .parse_script_options(&run_ctx)
        .map_err(|e| classify_runtime_error("failed to parse script options", e))?;

    run_ctx.thresholds = Arc::from(opts.thresholds.clone().into_boxed_slice());

    runtime
        .run_setup(&run_ctx)
        .map_err(|e| classify_runtime_error("script Setup failed", e))?;

    let scenarios = wrkr_core::scenarios_from_options(opts.clone(), cfg).map_err(|e| {
        RunError::InvalidInput(anyhow::Error::new(e).context("invalid scenario config"))
    })?;

    out.print_header(args.script.as_path(), &scenarios);
    let progress = out.progress();

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

        if matches!(args.output, OutputFormat::HumanReadable) {
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

    let checks_failed = summary.scenarios.iter().any(|s| s.checks_failed_total > 0);
    let thresholds_failed = !summary.threshold_violations.is_empty();

    Ok(ExitCode::from_quality_gates(
        checks_failed,
        thresholds_failed,
    ))
}

fn merged_env(overrides: &[String]) -> anyhow::Result<wrkr_core::EnvVars> {
    let mut map: BTreeMap<String, String> = std::env::vars()
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

fn classify_runtime_create_error(err: anyhow::Error) -> RunError {
    // Unsupported extensions and missing script files are treated as invalid input.
    if let Some(io) = err.downcast_ref::<std::io::Error>()
        && io.kind() == std::io::ErrorKind::NotFound
    {
        return RunError::InvalidInput(err);
    }
    RunError::InvalidInput(err)
}

fn classify_runtime_error(context: &'static str, err: crate::runtime::RuntimeError) -> RunError {
    #[cfg(feature = "lua")]
    {
        match err {
            crate::runtime::RuntimeError::Lua(lua_err) => {
                use wrkr_lua::Error as LuaError;

                let kind = match &lua_err {
                    // Invalid options/config input.
                    LuaError::InvalidIterations
                    | LuaError::InvalidVus
                    | LuaError::InvalidExecutor
                    | LuaError::InvalidStages
                    | LuaError::InvalidDuration
                    | LuaError::InvalidTimeUnit
                    | LuaError::InvalidScenarioTags
                    | LuaError::InvalidThresholds => RunError::InvalidInput,

                    // User script error (runtime error, missing entrypoints, bad API use).
                    LuaError::Lua(_)
                    | LuaError::MissingDefault
                    | LuaError::MissingExec(_)
                    | LuaError::MissingScriptPath(_)
                    | LuaError::InvalidPath(_)
                    | LuaError::InvalidMetricName
                    | LuaError::InvalidMetricValue => RunError::ScriptError,

                    // Core errors surfaced through the Lua layer.
                    LuaError::Core(_) => RunError::InvalidInput,

                    // IO while executing script hooks/modules.
                    LuaError::Io(_) => RunError::RuntimeError,
                };

                kind(anyhow::Error::new(lua_err).context(context))
            }
        }
    }

    #[cfg(not(feature = "lua"))]
    {
        let _ = context;
        RunError::RuntimeError(anyhow::Error::new(err))
    }
}
