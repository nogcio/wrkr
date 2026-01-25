use std::collections::BTreeMap;
use std::sync::Arc;

use crate::cli::OutputFormat;
use crate::cli::RunArgs;
use crate::output;
use crate::runtime;
use anyhow::Context as _;

pub async fn run(args: RunArgs) -> anyhow::Result<()> {
    let out = output::formatter(args.output);

    let env = merged_env(&args.env)?;
    let cfg = wrkr_core::RunConfig {
        iterations: args.iterations,
        vus: args.vus,
        duration: args.duration,
    };

    let runtime = runtime::create_runtime(&args.script)?;
    let run_ctx = runtime.create_run_context(&env);

    let opts = runtime
        .parse_script_options(&run_ctx)
        .context("failed to parse script options")?;

    runtime.run_setup(&run_ctx).context("script Setup failed")?;

    let scenarios =
        wrkr_core::scenarios_from_options(opts, cfg).context("invalid scenario config")?;

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
    .context("script run failed")?;

    runtime
        .run_teardown(&run_ctx)
        .context("script Teardown failed")?;

    let outputs = runtime.run_handle_summary(&run_ctx)?;
    if let Some(outputs) = outputs {
        let cwd = std::env::current_dir().context("failed to resolve current working directory")?;
        wrkr_core::write_output_files(&cwd, &outputs.files)
            .context("failed to write HandleSummary output files")?;

        if matches!(args.output, OutputFormat::HumanReadable) {
            if let Some(s) = outputs.stdout {
                print!("{s}");
            }
            if let Some(s) = outputs.stderr {
                eprint!("{s}");
            }
        }
    }

    out.print_summary(&summary)?;
    Ok(())
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
