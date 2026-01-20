use std::path::Path;
use std::path::PathBuf;
use std::sync::Arc;

use wrkr_testserver::TestServer;

fn read_test_script(name: &str) -> (PathBuf, String) {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/scripts")
        .join(name);

    let script = match std::fs::read_to_string(&path) {
        Ok(v) => v,
        Err(err) => panic!("failed to read lua script {}: {err}", path.display()),
    };

    (path, script)
}

async fn run_with_base_url(
    script_path: &Path,
    script: &str,
    base_url: &str,
) -> wrkr_core::runner::RunSummary {
    let env: wrkr_core::runner::EnvVars = Arc::from(
        vec![(Arc::<str>::from("BASE_URL"), Arc::<str>::from(base_url))].into_boxed_slice(),
    );
    let shared = Arc::new(wrkr_core::runner::SharedStore::default());
    let options_client = Arc::new(wrkr_core::HttpClient::default());
    let options_stats = Arc::new(wrkr_core::runner::RunStats::default());

    let opts = match wrkr_lua::parse_script_options(
        script,
        Some(script_path),
        &env,
        options_client,
        options_stats,
        shared.clone(),
    ) {
        Ok(v) => v,
        Err(err) => panic!("parse_script_options failed: {err}"),
    };
    let scenarios =
        match wrkr_core::runner::scenarios_from_options(opts, wrkr_lua::RunConfig::default()) {
            Ok(v) => v,
            Err(err) => panic!("scenarios_from_options failed: {err}"),
        };
    match wrkr_core::runner::run_scenarios(
        script,
        Some(script_path),
        scenarios,
        env,
        shared,
        wrkr_lua::run_vu,
        None,
    )
    .await
    {
        Ok(v) => v,
        Err(err) => panic!("run_scenarios failed: {err}"),
    }
}

#[tokio::test]
async fn lua_script_hits_plaintext_and_checks_body() {
    let server = TestServer::start()
        .await
        .unwrap_or_else(|err| panic!("start test server failed: {err}"));
    let base_url = server.base_url().to_string();
    let (script_path, script) = read_test_script("plaintext.lua");

    let summary = run_with_base_url(&script_path, &script, &base_url).await;
    assert_eq!(summary.requests_total, 1);
    assert_eq!(summary.checks_total, 2);
    assert_eq!(summary.checks_failed, 0);
    assert_eq!(summary.checks_failed, 0);

    server.shutdown().await;
}

#[tokio::test]
async fn scenarios_shared_iterations_across_vus() {
    let server = TestServer::start()
        .await
        .unwrap_or_else(|err| panic!("start test server failed: {err}"));
    let base_url = server.base_url().to_string();
    let (script_path, script) = read_test_script("shared_iterations.lua");

    let summary = run_with_base_url(&script_path, &script, &base_url).await;

    assert_eq!(summary.requests_total, 10);
    assert_eq!(summary.checks_total, 10);
    assert_eq!(summary.checks_failed, 0);
    assert_eq!(summary.checks_failed, 0);

    server.shutdown().await;
}

#[tokio::test]
async fn http_post_sends_header_and_body() {
    let server = TestServer::start()
        .await
        .unwrap_or_else(|err| panic!("start test server failed: {err}"));
    let base_url = server.base_url().to_string();
    let (script_path, script) = read_test_script("post_echo.lua");

    let summary = run_with_base_url(&script_path, &script, &base_url).await;

    assert_eq!(summary.requests_total, 1);
    assert_eq!(summary.checks_failed, 0);
    assert_eq!(server.stats().saw_post_header(), 1);
    assert_eq!(server.stats().saw_post_body(), 1);

    server.shutdown().await;
}

#[tokio::test]
async fn http_post_table_body_is_json_and_sets_content_type() {
    let server = TestServer::start()
        .await
        .unwrap_or_else(|err| panic!("start test server failed: {err}"));
    let base_url = server.base_url().to_string();
    let (script_path, script) = read_test_script("post_json.lua");

    let summary = run_with_base_url(&script_path, &script, &base_url).await;

    assert_eq!(summary.requests_total, 1);
    assert_eq!(summary.checks_failed, 0);
    assert_eq!(server.stats().saw_post_header(), 1);
    assert_eq!(server.stats().saw_json_content_type(), 1);

    server.shutdown().await;
}

#[tokio::test]
async fn duration_mode_runs_until_deadline() {
    let server = TestServer::start()
        .await
        .unwrap_or_else(|err| panic!("start test server failed: {err}"));
    let base_url = server.base_url().to_string();
    let (script_path, script) = read_test_script("duration.lua");

    let summary = run_with_base_url(&script_path, &script, &base_url).await;

    assert!(summary.run_duration_ms > 0);
    assert!(summary.requests_total > 0);
    assert_eq!(summary.checks_failed, 0);

    server.shutdown().await;
}

#[tokio::test]
async fn ramping_vus_executor_runs_and_sends_requests() {
    let server = TestServer::start()
        .await
        .unwrap_or_else(|err| panic!("start test server failed: {err}"));
    let base_url = server.base_url().to_string();
    let (script_path, script) = read_test_script("ramping_vus.lua");

    let summary = run_with_base_url(&script_path, &script, &base_url).await;

    assert!(summary.requests_total > 0);
    assert_eq!(summary.checks_failed, 0);

    server.shutdown().await;
}

#[tokio::test]
async fn ramping_arrival_rate_executor_runs_and_sends_requests() {
    let server = TestServer::start()
        .await
        .unwrap_or_else(|err| panic!("start test server failed: {err}"));
    let base_url = server.base_url().to_string();
    let (script_path, script) = read_test_script("ramping_arrival_rate.lua");

    let summary = run_with_base_url(&script_path, &script, &base_url).await;

    assert!(summary.requests_total > 0);
    assert_eq!(summary.checks_failed, 0);

    server.shutdown().await;
}

#[tokio::test]
async fn http_timeout_returns_error_response_not_lua_error() {
    let server = TestServer::start()
        .await
        .unwrap_or_else(|err| panic!("start test server failed: {err}"));
    let base_url = server.base_url().to_string();
    let (script_path, script) = read_test_script("timeout.lua");

    let summary = run_with_base_url(&script_path, &script, &base_url).await;

    assert_eq!(summary.requests_total, 1);
    assert_eq!(summary.checks_failed, 1);

    server.shutdown().await;
}

#[tokio::test]
async fn http_get_supports_query_params() {
    let server = TestServer::start()
        .await
        .unwrap_or_else(|err| panic!("start test server failed: {err}"));
    let base_url = server.base_url().to_string();
    let (script_path, script) = read_test_script("query_params.lua");

    let summary = run_with_base_url(&script_path, &script, &base_url).await;

    assert_eq!(summary.requests_total, 1);
    assert_eq!(summary.checks_failed, 0);

    server.shutdown().await;
}

#[tokio::test]
async fn lua_script_can_require_local_modules_and_open_files() {
    let server = TestServer::start()
        .await
        .unwrap_or_else(|err| panic!("start test server failed: {err}"));
    let base_url = server.base_url().to_string();
    let (script_path, script) = read_test_script("modular.lua");

    let summary = run_with_base_url(&script_path, &script, &base_url).await;

    assert_eq!(summary.requests_total, 1);
    assert_eq!(summary.checks_failed, 0);

    server.shutdown().await;
}

#[tokio::test]
async fn lua_script_can_require_wrkr_style_modules() {
    let server = TestServer::start()
        .await
        .unwrap_or_else(|err| panic!("start test server failed: {err}"));
    let base_url = server.base_url().to_string();
    let (script_path, script) = read_test_script("wrkr_style.lua");

    let summary = run_with_base_url(&script_path, &script, &base_url).await;

    assert_eq!(summary.requests_total, 1);
    assert_eq!(summary.checks_failed, 0);
    assert_eq!(summary.checks_failed, 0);

    server.shutdown().await;
}

#[tokio::test]
async fn shared_store_module_end_to_end() {
    let server = TestServer::start()
        .await
        .unwrap_or_else(|err| panic!("start test server failed: {err}"));
    let base_url = server.base_url().to_string();
    let (script_path, script) = read_test_script("shared_store.lua");

    let summary = match tokio::time::timeout(
        std::time::Duration::from_secs(2),
        run_with_base_url(&script_path, &script, &base_url),
    )
    .await
    {
        Ok(v) => v,
        Err(_) => panic!("shared_store_module_end_to_end timed out"),
    };

    assert_eq!(summary.checks_failed, 0);
    assert_eq!(summary.requests_total, 0);

    server.shutdown().await;
}

#[tokio::test]
async fn shared_store_module_multi_vu_end_to_end() {
    let server = TestServer::start()
        .await
        .unwrap_or_else(|err| panic!("start test server failed: {err}"));
    let base_url = server.base_url().to_string();
    let (script_path, script) = read_test_script("shared_store_multi_vu.lua");

    let summary = match tokio::time::timeout(
        std::time::Duration::from_secs(3),
        run_with_base_url(&script_path, &script, &base_url),
    )
    .await
    {
        Ok(v) => v,
        Err(_) => panic!("shared_store_module_multi_vu_end_to_end timed out"),
    };

    assert_eq!(summary.checks_failed, 0);
    assert_eq!(summary.requests_total, 0);

    server.shutdown().await;
}
