use std::path::Path;
use std::process::Command;

use anyhow::Context as _;
use wrkr_testserver::TestServer;

#[tokio::test]
async fn e2e_lua_runs_for_2s_and_sends_requests() -> anyhow::Result<()> {
    let server = TestServer::start().await.context("start test server")?;
    let base_url = server.base_url().to_string();

    let script_path = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/scripts/hello_world.lua");

    let exe = env!("CARGO_BIN_EXE_wrkr");

    let output = tokio::task::spawn_blocking(move || {
        Command::new(exe)
            .arg("run")
            .arg(&script_path)
            .arg("--duration")
            .arg("2s")
            .arg("--output")
            .arg("json")
            .env("BASE_URL", &base_url)
            .output()
    })
    .await
    .context("spawn_blocking join")?
    .context("run wrkr binary")?;

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

    let last_line = stdout
        .lines()
        .rev()
        .find(|l| !l.trim().is_empty())
        .unwrap_or("");
    let v: serde_json::Value = serde_json::from_str(last_line)
        .with_context(|| format!("failed to parse json progress line: {last_line}"))?;
    anyhow::ensure!(
        v.get("elapsed_secs").is_some(),
        "expected a progress json object with `elapsed_secs` key\nstdout:\n{}\nstderr:\n{}",
        stdout,
        stderr
    );

    anyhow::ensure!(
        server_seen > 0,
        "expected server to see requests\nserver_seen={}\nstdout:\n{}\nstderr:\n{}",
        server_seen,
        stdout,
        stderr
    );

    Ok(())
}
