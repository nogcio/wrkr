#![allow(dead_code)]

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use wrkr_lua::Result;

pub struct LoadedScript {
    pub path: PathBuf,
    pub text: String,
}

pub fn scripts_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("scripts")
}

pub fn load_test_script(name: &str) -> std::io::Result<LoadedScript> {
    let path = scripts_dir().join(name);
    let text = std::fs::read_to_string(&path)?;
    Ok(LoadedScript { path, text })
}

pub fn env_with(overrides: &[(&str, String)]) -> wrkr_core::EnvVars {
    let mut map: BTreeMap<String, String> = std::env::vars().collect();
    for (k, v) in overrides {
        map.insert((*k).to_string(), v.clone());
    }

    let vars: Vec<(Arc<str>, Arc<str>)> = map
        .into_iter()
        .map(|(k, v)| (Arc::<str>::from(k), Arc::<str>::from(v)))
        .collect();

    Arc::from(vars.into_boxed_slice())
}

pub fn run_ctx_for_script(
    script: &LoadedScript,
    env: wrkr_core::EnvVars,
) -> wrkr_core::RunScenariosContext {
    wrkr_core::RunScenariosContext::new(env, script.text.clone(), script.path.clone())
}

pub async fn run_script(
    script_name: &str,
    env_overrides: &[(&str, String)],
    cfg: wrkr_core::RunConfig,
) -> Result<wrkr_core::RunSummary> {
    let script = load_test_script(script_name)?;
    let env = env_with(env_overrides);
    let run_ctx = run_ctx_for_script(&script, env);

    let opts = wrkr_lua::parse_script_options(&run_ctx)?;
    let scenarios = wrkr_core::scenarios_from_options(opts, cfg)?;

    let summary = wrkr_core::run_scenarios(scenarios, run_ctx, wrkr_lua::run_vu, None).await?;
    Ok(summary)
}
