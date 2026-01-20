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

fn write_temp_script(dir: &Path, script: &str) -> std::path::PathBuf {
    let script_path = dir.join("script.lua");
    std::fs::write(&script_path, script).unwrap_or_else(|e| panic!("write script failed: {e}"));
    script_path
}

fn tags_contain_all(haystack: &[(String, String)], expected: &[(&str, &str)]) -> bool {
    expected
        .iter()
        .all(|(k, v)| haystack.iter().any(|(hk, hv)| hk == k && hv == v))
}

#[tokio::test]
async fn metrics_and_group_tags_are_recorded() {
    let server = TestServer::start()
        .await
        .unwrap_or_else(|err| panic!("start test server failed: {err}"));
    let base_url = server.base_url().to_string();

    let script = read_repo_test_script("metrics_tags.lua");
    let dir = make_temp_dir("lua_metrics_tags");
    let script_path = write_temp_script(&dir, &script);

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

    let scenarios = wrkr_core::runner::scenarios_from_options(opts, wrkr_lua::RunConfig::default())
        .unwrap_or_else(|e| panic!("scenarios_from_options failed: {e}"));

    let summary = wrkr_core::runner::run_scenarios(
        &script,
        Some(&script_path),
        scenarios,
        env,
        shared,
        wrkr_lua::run_vu,
        None,
    )
    .await
    .unwrap_or_else(|e| panic!("run_scenarios failed: {e}"));

    // HTTP metrics should include group tag from group.group(), and preserve explicit group tag.
    let http_reqs = summary
        .metrics
        .iter()
        .filter(|m| m.name == "http_reqs")
        .collect::<Vec<_>>();
    assert!(!http_reqs.is_empty());

    let has_group_injected = http_reqs.iter().any(|m| {
        tags_contain_all(
            &m.tags,
            &[
                ("method", "GET"),
                ("name", "GET /plaintext"),
                ("status", "200"),
                ("scenario", "main"),
                ("group", "g_http"),
            ],
        )
    });
    assert!(has_group_injected);

    let has_group_preserved = http_reqs.iter().any(|m| {
        tags_contain_all(
            &m.tags,
            &[
                ("method", "GET"),
                ("name", "GET /plaintext override"),
                ("status", "200"),
                ("scenario", "main"),
                ("group", "manual"),
            ],
        )
    });
    assert!(has_group_preserved);

    // Custom metrics should get group tag injected too.
    let custom_counter = summary
        .metrics
        .iter()
        .find(|m| {
            m.name == "custom_counter"
                && tags_contain_all(&m.tags, &[("k", "v"), ("group", "g_metric")])
        })
        .unwrap_or_else(|| panic!("missing custom_counter series with expected tags"));
    assert!(matches!(
        custom_counter.values,
        wrkr_core::runner::MetricValues::Counter { .. }
    ));

    let custom_trend = summary
        .metrics
        .iter()
        .find(|m| {
            m.name == "custom_trend_tagged"
                && tags_contain_all(&m.tags, &[("scenario", "main"), ("group", "g_metric")])
        })
        .unwrap_or_else(|| panic!("missing custom_trend_tagged series with expected tags"));
    assert!(matches!(
        custom_trend.values,
        wrkr_core::runner::MetricValues::Trend { .. }
    ));

    let _ = std::fs::remove_dir_all(&dir);
    server.shutdown().await;
}

#[tokio::test]
async fn invalid_metric_name_fails_vu() {
    let script = read_repo_test_script("invalid_metric_name.lua");
    let dir = make_temp_dir("lua_invalid_metric_name");
    let script_path = write_temp_script(&dir, &script);

    let env: wrkr_core::runner::EnvVars =
        Arc::from(Vec::<(Arc<str>, Arc<str>)>::new().into_boxed_slice());

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

    let scenarios = wrkr_core::runner::scenarios_from_options(opts, wrkr_lua::RunConfig::default())
        .unwrap_or_else(|e| panic!("scenarios_from_options failed: {e}"));

    let err = wrkr_core::runner::run_scenarios(
        &script,
        Some(&script_path),
        scenarios,
        env,
        shared,
        wrkr_lua::run_vu,
        None,
    )
    .await
    .err()
    .unwrap_or_else(|| panic!("expected run_scenarios to fail"));

    let msg = err.to_string();
    assert!(
        msg.contains("invalid metric name"),
        "unexpected error: {msg}"
    );

    let _ = std::fs::remove_dir_all(&dir);
}

#[tokio::test]
async fn invalid_http_name_fails_vu() {
    let server = TestServer::start()
        .await
        .unwrap_or_else(|err| panic!("start test server failed: {err}"));
    let base_url = server.base_url().to_string();

    let script = read_repo_test_script("invalid_http_name.lua");
    let dir = make_temp_dir("lua_invalid_http_name");
    let script_path = write_temp_script(&dir, &script);

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

    let scenarios = wrkr_core::runner::scenarios_from_options(opts, wrkr_lua::RunConfig::default())
        .unwrap_or_else(|e| panic!("scenarios_from_options failed: {e}"));

    let err = wrkr_core::runner::run_scenarios(
        &script,
        Some(&script_path),
        scenarios,
        env,
        shared,
        wrkr_lua::run_vu,
        None,
    )
    .await
    .err()
    .unwrap_or_else(|| panic!("expected run_scenarios to fail"));

    let msg = err.to_string();
    assert!(
        msg.contains("invalid http option `name`") || msg.contains("InvalidHttpName"),
        "unexpected error: {msg}"
    );

    let _ = std::fs::remove_dir_all(&dir);
    server.shutdown().await;
}
