use std::path::Path;

use anyhow::Context as _;
use wrkr_testserver::TestServer;

#[tokio::test]
async fn e2e_dashboard_out_writes_self_contained_html() -> anyhow::Result<()> {
    let server = TestServer::start().await.context("start test server")?;
    let base_url = server.base_url().to_string();

    let tmp = tempfile::tempdir().context("create tempdir")?;
    let out_path = tmp.path().join("dashboard.html");

    let script_path = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/scripts/hello_world.lua");
    let exe = env!("CARGO_BIN_EXE_wrkr");

    let status = tokio::process::Command::new(exe)
        .arg("run")
        .arg(&script_path)
        .arg("--duration")
        .arg("2s")
        .arg("--output")
        .arg("json")
        .arg("--dashboard-out")
        .arg(&out_path)
        .env("BASE_URL", &base_url)
        .status()
        .await
        .context("run wrkr")?;

    let server_seen = server.stats().requests_total();
    server.shutdown().await;

    anyhow::ensure!(status.success(), "wrkr exited with {status}");
    anyhow::ensure!(server_seen > 0, "expected server to see requests");

    let html: String = tokio::fs::read_to_string(&out_path)
        .await
        .with_context(|| format!("read dashboard file: {}", out_path.display()))?;
    anyhow::ensure!(
        html.contains(r#"<title>wrkr — progress</title>"#),
        "offline html is missing title"
    );
    anyhow::ensure!(
        html.contains(r#"id="wrkrSnapshot""#),
        "offline html is missing embedded snapshot"
    );
    anyhow::ensure!(
        html.contains(r#""type":"snapshot""#),
        "offline html is missing snapshot json"
    );

    Ok(())
}
