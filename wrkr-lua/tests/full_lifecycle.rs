mod support;

use std::path::PathBuf;

use wrkr_lua::Result;
use wrkr_testserver::TestServer;

fn temp_dir(name: &str) -> PathBuf {
    let id = uuid::Uuid::new_v4();
    std::env::temp_dir().join(format!("wrkr-lua-{name}-{id}"))
}

#[tokio::test]
async fn full_lifecycle_script_runs_setup_vus_teardown_and_handle_summary() -> Result<()> {
    let server = TestServer::start().await?;

    let script = support::load_test_script("lifecycle.lua")?;
    let env = support::env_with(&[("BASE_URL", server.base_url().to_string())]);
    let run_ctx = support::run_ctx_for_script(&script, env);

    let opts = wrkr_lua::parse_script_options(&run_ctx)?;
    let scenarios = wrkr_core::scenarios_from_options(opts, wrkr_core::RunConfig::default())?;

    wrkr_lua::run_setup(&run_ctx)?;
    let _summary =
        wrkr_core::run_scenarios(scenarios, run_ctx.clone(), wrkr_lua::run_vu, None).await?;
    wrkr_lua::run_teardown(&run_ctx)?;

    let out = wrkr_lua::run_handle_summary(&run_ctx)?;
    let Some(out) = out else {
        panic!("expected HandleSummary outputs");
    };

    let base = temp_dir("lifecycle");
    std::fs::create_dir_all(&base).unwrap_or_else(|err| panic!("create temp dir: {err}"));
    wrkr_core::write_output_files(&base, &out.files)?;

    let txt_path = base.join("summary.txt");
    let raw =
        std::fs::read_to_string(&txt_path).unwrap_or_else(|err| panic!("read summary.txt: {err}"));
    assert_eq!(raw, "ok\n");

    let seen = server.stats().requests_total();
    server.shutdown().await;

    assert!(seen > 0, "expected server to see requests");
    Ok(())
}
