use std::path::Path;
use std::sync::Arc;

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

fn write_temp_script(dir: &Path, script: &str) -> std::path::PathBuf {
    let script_path = dir.join("script.lua");
    std::fs::write(&script_path, script).unwrap_or_else(|e| panic!("write script failed: {e}"));
    script_path
}

async fn run_script(
    script: &str,
    script_path: &Path,
) -> (
    wrkr_core::runner::RunSummary,
    Arc<wrkr_core::runner::SharedStore>,
) {
    let env: wrkr_core::runner::EnvVars =
        Arc::from(Vec::<(Arc<str>, Arc<str>)>::new().into_boxed_slice());

    let shared = Arc::new(wrkr_core::runner::SharedStore::default());

    let options_client = Arc::new(wrkr_core::HttpClient::default());
    let options_stats = Arc::new(wrkr_core::runner::RunStats::default());
    let opts = wrkr_lua::parse_script_options(
        script,
        Some(script_path),
        &env,
        options_client,
        options_stats,
        shared.clone(),
    )
    .unwrap_or_else(|e| panic!("parse_script_options failed: {e}"));

    let scenarios = wrkr_core::runner::scenarios_from_options(opts, wrkr_lua::RunConfig::default())
        .unwrap_or_else(|e| panic!("scenarios_from_options failed: {e}"));

    let summary = wrkr_core::runner::run_scenarios(
        script,
        Some(script_path),
        scenarios,
        env,
        shared.clone(),
        wrkr_lua::run_vu,
        None,
    )
    .await
    .unwrap_or_else(|e| panic!("run_scenarios failed: {e}"));

    (summary, shared)
}

#[tokio::test]
async fn handle_summary_writes_files_and_captures_stdio() {
    let script = read_repo_test_script("handle_summary_outputs.lua");
    let dir = make_temp_dir("lua_handle_summary_outputs");
    let script_path = write_temp_script(&dir, &script);

    let (summary, shared) = run_script(&script, &script_path).await;

    let env: wrkr_core::runner::EnvVars =
        Arc::from(Vec::<(Arc<str>, Arc<str>)>::new().into_boxed_slice());
    let outputs = wrkr_lua::run_handle_summary(&script, Some(&script_path), &env, &summary, shared)
        .unwrap_or_else(|e| panic!("run_handle_summary failed: {e}"))
        .unwrap_or_else(|| panic!("expected HandleSummary outputs"));

    assert_eq!(outputs.stdout.as_deref(), Some("hello\n"));
    assert_eq!(outputs.stderr.as_deref(), Some("oops\n"));

    wrkr_core::runner::write_output_files(&dir, &outputs.files)
        .unwrap_or_else(|e| panic!("write_output_files failed: {e}"));

    let written = std::fs::read_to_string(dir.join("dir/out.txt"))
        .unwrap_or_else(|e| panic!("expected output file to be written: {e}"));
    assert_eq!(written, "ok\n");

    let _ = std::fs::remove_dir_all(&dir);
}

#[tokio::test]
async fn handle_summary_rejects_path_traversal() {
    let script = read_repo_test_script("handle_summary_invalid_path.lua");
    let dir = make_temp_dir("lua_handle_summary_invalid_path");
    let script_path = write_temp_script(&dir, &script);

    let (summary, shared) = run_script(&script, &script_path).await;

    let env: wrkr_core::runner::EnvVars =
        Arc::from(Vec::<(Arc<str>, Arc<str>)>::new().into_boxed_slice());

    let outputs = wrkr_lua::run_handle_summary(&script, Some(&script_path), &env, &summary, shared)
        .unwrap_or_else(|e| panic!("run_handle_summary failed: {e}"))
        .unwrap_or_else(|| panic!("expected HandleSummary outputs"));

    let err = wrkr_core::runner::write_output_files(&dir, &outputs.files)
        .err()
        .unwrap_or_else(|| panic!("expected write_output_files to fail"));

    let msg = err.to_string();
    assert!(
        msg.contains("invalid output path"),
        "unexpected error: {msg}"
    );

    let _ = std::fs::remove_dir_all(&dir);
}

#[tokio::test]
async fn handle_summary_non_table_return_is_ok() {
    let script = read_repo_test_script("handle_summary_non_table.lua");
    let dir = make_temp_dir("lua_handle_summary_non_table");
    let script_path = write_temp_script(&dir, &script);

    let (summary, shared) = run_script(&script, &script_path).await;

    let env: wrkr_core::runner::EnvVars =
        Arc::from(Vec::<(Arc<str>, Arc<str>)>::new().into_boxed_slice());
    let outputs = wrkr_lua::run_handle_summary(&script, Some(&script_path), &env, &summary, shared)
        .unwrap_or_else(|e| panic!("run_handle_summary failed: {e}"))
        .unwrap_or_else(|| panic!("expected HandleSummary outputs"));

    assert!(outputs.stdout.is_none());
    assert!(outputs.stderr.is_none());
    assert!(outputs.files.is_empty());

    let _ = std::fs::remove_dir_all(&dir);
}

#[tokio::test]
async fn handle_summary_includes_checks_by_name() {
    let script = read_repo_test_script("handle_summary_checks_by_name.lua");
    let dir = make_temp_dir("lua_handle_summary_checks_by_name");
    let script_path = write_temp_script(&dir, &script);

    let (summary, shared) = run_script(&script, &script_path).await;

    let env: wrkr_core::runner::EnvVars =
        Arc::from(Vec::<(Arc<str>, Arc<str>)>::new().into_boxed_slice());
    let outputs = wrkr_lua::run_handle_summary(&script, Some(&script_path), &env, &summary, shared)
        .unwrap_or_else(|e| panic!("run_handle_summary failed: {e}"))
        .unwrap_or_else(|| panic!("expected HandleSummary outputs"));

    wrkr_core::runner::write_output_files(&dir, &outputs.files)
        .unwrap_or_else(|e| panic!("write_output_files failed: {e}"));

    let raw = std::fs::read_to_string(dir.join("summary.json"))
        .unwrap_or_else(|e| panic!("expected summary.json to be written: {e}"));
    let v: serde_json::Value =
        serde_json::from_str(&raw).unwrap_or_else(|e| panic!("invalid json: {e}"));

    assert_eq!(v["checks_total"].as_u64(), Some(2));
    assert_eq!(v["checks_failed"].as_u64(), Some(1));

    let checks = v["checks_by_name"].as_array().unwrap_or_else(|| {
        panic!(
            "expected checks_by_name array, got: {}",
            v["checks_by_name"]
        )
    });
    let mut by_name: std::collections::HashMap<&str, (u64, u64)> = std::collections::HashMap::new();
    for c in checks {
        let name = c["name"].as_str().unwrap_or_default();
        let total = c["total"].as_u64().unwrap_or_default();
        let failed = c["failed"].as_u64().unwrap_or_default();
        by_name.insert(name, (total, failed));
    }

    assert_eq!(by_name.get("check ok"), Some(&(1, 0)));
    assert_eq!(by_name.get("check fail"), Some(&(1, 1)));

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn setup_nil_returns_none_and_teardown_accepts_nil() {
    let script = read_repo_test_script("setup_nil.lua");
    let dir = make_temp_dir("lua_setup_nil");
    let script_path = write_temp_script(&dir, &script);

    let env: wrkr_core::runner::EnvVars =
        Arc::from(Vec::<(Arc<str>, Arc<str>)>::new().into_boxed_slice());

    let shared = Arc::new(wrkr_core::runner::SharedStore::default());

    wrkr_lua::run_setup(&script, Some(&script_path), &env, shared.clone())
        .unwrap_or_else(|e| panic!("run_setup failed: {e}"));

    wrkr_lua::run_teardown(&script, Some(&script_path), &env, shared)
        .unwrap_or_else(|e| panic!("run_teardown failed: {e}"));

    let _ = std::fs::remove_dir_all(&dir);
}
