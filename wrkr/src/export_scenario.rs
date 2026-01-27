use crate::cli::ExportScenarioArgs;
use crate::exit_codes::ExitCode;
use crate::run_error::RunError;
use crate::run_support::{classify_runtime_create_error, classify_runtime_error, merged_env};
use crate::runtime;
use crate::scenario_yaml;

pub async fn export_scenario(args: ExportScenarioArgs) -> Result<ExitCode, RunError> {
    let env = merged_env(&args.env).map_err(RunError::InvalidInput)?;
    let cfg = wrkr_core::RunConfig {
        iterations: args.iterations,
        vus: args.vus,
        duration: args.duration,
    };

    let runtime = runtime::create_runtime(&args.script).map_err(classify_runtime_create_error)?;
    let run_ctx = runtime.create_run_context(&env);

    let opts = runtime
        .parse_script_options(&run_ctx)
        .map_err(|e| classify_runtime_error("failed to parse script options", e))?;

    let scenarios = wrkr_core::scenarios_from_options(opts.clone(), cfg).map_err(|e| {
        RunError::InvalidInput(anyhow::Error::new(e).context("invalid scenario config"))
    })?;

    let doc = scenario_yaml::build_doc_from_resolved_scenarios(&scenarios, &opts.thresholds);
    scenario_yaml::write_yaml_file(&args.out, &doc)
        .await
        .map_err(|e| RunError::RuntimeError(e.context("failed to write scenario YAML")))?;

    Ok(ExitCode::Success)
}
