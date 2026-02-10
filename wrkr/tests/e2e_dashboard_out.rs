use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::Context as _;
use wrkr_testserver::TestServer;

#[path = "support/helpers.rs"]
mod support;

#[tokio::test]
async fn e2e_dashboard_out_writes_single_html_file() -> anyhow::Result<()> {
    let server = TestServer::start().await.context("start test server")?;
    let base_url = server.base_url().to_string();

    let script_path = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/scripts/hello_world.lua");

    let ts = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis();
    let out_path = std::env::temp_dir().join(format!("wrkr-dashboard-{ts}.html"));

    let output = support::run_wrkr_output(
        [
            "run".to_string(),
            script_path.to_string_lossy().to_string(),
            "--duration".to_string(),
            "2s".to_string(),
            "--dashboard-out".to_string(),
            out_path.to_string_lossy().to_string(),
        ],
        [("BASE_URL", base_url.as_str())],
        std::time::Duration::from_secs(15),
    )
    .await
    .context("run wrkr")?;

    let server_seen = server.stats().requests_total();
    server.shutdown().await;

    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();

    anyhow::ensure!(
        output.status.success(),
        "wrkr exited with {}\nstdout:\n{}\nstderr:\n{}",
        output.status,
        stdout,
        stderr
    );

    anyhow::ensure!(
        out_path.exists(),
        "expected report file at {}",
        out_path.display()
    );

    let html = tokio::fs::read_to_string(&out_path)
        .await
        .context("read offline report")?;

    // Basic offline invariants: single HTML with embedded snapshot.
    anyhow::ensure!(
        html.contains("<script id=\"snapshot\""),
        "missing snapshot script tag"
    );
    anyhow::ensure!(
        html.contains("wrkr.dashboard.v1"),
        "missing dashboard schema marker"
    );
    anyhow::ensure!(
        !html.contains("<script src=") && !html.contains("<link rel="),
        "offline report should not reference external assets"
    );

    // Cleanup (best-effort).
    let _ = tokio::fs::remove_file(&out_path).await;

    anyhow::ensure!(server_seen > 0, "expected server to see requests");

    Ok(())
}
