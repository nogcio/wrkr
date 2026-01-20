use std::path::Path;
use std::sync::Arc;

use wrkr_testserver::TestServer;

fn make_temp_dir(name: &str) -> std::path::PathBuf {
    use std::time::{SystemTime, UNIX_EPOCH};

    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let pid = std::process::id();

    let dir = std::env::temp_dir().join(format!("wrkr_{name}_{pid}_{now}"));
    std::fs::create_dir_all(&dir).unwrap_or_else(|e| panic!("create temp dir failed: {e}"));
    dir
}

fn read_repo_test_script(name: &str) -> String {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/scripts")
        .join(name);
    std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("failed to read script {}: {e}", path.display()))
}

#[tokio::test]
async fn lifecycle_setup_teardown_and_handle_summary() {
    let server = TestServer::start()
        .await
        .unwrap_or_else(|err| panic!("start test server failed: {err}"));
    let base_url = server.base_url().to_string();

    let script = read_repo_test_script("lifecycle.lua");
    let dir = make_temp_dir("lua_lifecycle");
    let script_path = dir.join("script.lua");
    std::fs::write(&script_path, &script).unwrap_or_else(|e| panic!("write script failed: {e}"));

    let env: wrkr_core::runner::EnvVars = Arc::from(
        vec![(Arc::<str>::from("BASE_URL"), Arc::<str>::from(base_url))].into_boxed_slice(),
    );

    let shared = Arc::new(wrkr_core::runner::SharedStore::default());

    wrkr_lua::run_setup(&script, Some(&script_path), &env, shared.clone())
        .unwrap_or_else(|e| panic!("run_setup failed: {e}"));

    let options_client = Arc::new(wrkr_core::HttpClient::default());
    let options_stats = Arc::new(wrkr_core::runner::RunStats::default());

    let opts = wrkr_lua::parse_script_options(
        &script,
        Some(&script_path),
        &env,
        options_client,
        options_stats,
        shared.clone(),
    )
    .unwrap_or_else(|e| panic!("parse_script_options failed: {e}"));

    let scenarios = wrkr_core::runner::scenarios_from_options(opts, wrkr_lua::RunConfig::default())
        .unwrap_or_else(|e| panic!("scenarios_from_options failed: {e}"));

    let summary = wrkr_core::runner::run_scenarios(
        &script,
        Some(&script_path),
        scenarios,
        env.clone(),
        shared.clone(),
        wrkr_lua::run_vu,
        None,
    )
    .await
    .unwrap_or_else(|e| panic!("run_scenarios failed: {e}"));

    wrkr_lua::run_teardown(&script, Some(&script_path), &env, shared.clone())
        .unwrap_or_else(|e| panic!("run_teardown failed: {e}"));

    let outputs =
        wrkr_lua::run_handle_summary(&script, Some(&script_path), &env, &summary, shared.clone())
            .unwrap_or_else(|e| panic!("run_handle_summary failed: {e}"));
    assert!(outputs.is_some());

    let outputs = outputs.unwrap_or_else(|| panic!("expected HandleSummary outputs"));
    wrkr_core::runner::write_output_files(&dir, &outputs.files)
        .unwrap_or_else(|e| panic!("write_output_files failed: {e}"));

    let summary_path = dir.join("summary.txt");
    let written = std::fs::read_to_string(&summary_path).unwrap_or_else(|e| {
        panic!(
            "expected summary output file {}: {e}",
            summary_path.display()
        )
    });
    assert_eq!(written, "ok\n");

    let _ = std::fs::remove_dir_all(&dir);
    server.shutdown().await;
}

#[tokio::test]
async fn thresholds_parse_and_evaluate() {
    let server = TestServer::start()
        .await
        .unwrap_or_else(|err| panic!("start test server failed: {err}"));
    let base_url = server.base_url().to_string();

    let script = read_repo_test_script("thresholds.lua");
    let dir = make_temp_dir("lua_thresholds");
    let script_path = dir.join("script.lua");
    std::fs::write(&script_path, &script).unwrap_or_else(|e| panic!("write script failed: {e}"));

    let env: wrkr_core::runner::EnvVars = Arc::from(
        vec![(Arc::<str>::from("BASE_URL"), Arc::<str>::from(base_url))].into_boxed_slice(),
    );

    let shared = Arc::new(wrkr_core::runner::SharedStore::default());

    let options_client = Arc::new(wrkr_core::HttpClient::default());
    let options_stats = Arc::new(wrkr_core::runner::RunStats::default());

    let opts = wrkr_lua::parse_script_options(
        &script,
        Some(&script_path),
        &env,
        options_client,
        options_stats,
        shared.clone(),
    )
    .unwrap_or_else(|e| panic!("parse_script_options failed: {e}"));

    assert_eq!(opts.thresholds.len(), 1);

    let thresholds = opts.thresholds.clone();
    let scenarios = wrkr_core::runner::scenarios_from_options(opts, wrkr_lua::RunConfig::default())
        .unwrap_or_else(|e| panic!("scenarios_from_options failed: {e}"));

    let summary = wrkr_core::runner::run_scenarios(
        &script,
        Some(&script_path),
        scenarios,
        env.clone(),
        shared,
        wrkr_lua::run_vu,
        None,
    )
    .await
    .unwrap_or_else(|e| panic!("run_scenarios failed: {e}"));

    let violations = wrkr_core::runner::evaluate_thresholds(&thresholds, &summary.metrics)
        .unwrap_or_else(|e| panic!("evaluate_thresholds failed: {e}"));
    assert_eq!(violations.len(), 1);

    let _ = std::fs::remove_dir_all(&dir);
    server.shutdown().await;
}
