use std::path::Path;
use std::path::PathBuf;
use std::sync::Arc;

use wrkr_testserver::GrpcTestServer;

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

async fn run_with_env(
    script_path: &Path,
    script: &str,
    env_kv: Vec<(Arc<str>, Arc<str>)>,
) -> wrkr_core::runner::RunSummary {
    let env: wrkr_core::runner::EnvVars = Arc::from(env_kv.into_boxed_slice());
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
async fn grpc_unary_roundtrip() {
    let server = GrpcTestServer::start()
        .await
        .unwrap_or_else(|err| panic!("start grpc test server failed: {err}"));
    let target = server.target().to_string();

    let (script_path, script) = read_test_script("grpc_unary.lua");

    let summary = run_with_env(
        &script_path,
        &script,
        vec![(Arc::<str>::from("GRPC_TARGET"), Arc::<str>::from(target))],
    )
    .await;

    assert_eq!(summary.requests_total, 1);
    assert_eq!(summary.checks_failed, 0);

    server.shutdown().await;
}
