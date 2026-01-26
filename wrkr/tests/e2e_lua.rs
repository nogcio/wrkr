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

    let mut saw_progress = false;
    let mut saw_summary = false;
    let mut last_progress_line = String::new();

    for line in stdout.lines().filter(|l| !l.trim().is_empty()) {
        let v: serde_json::Value = serde_json::from_str(line)
            .with_context(|| format!("failed to parse json line: {line}"))?;

        match v.get("kind").and_then(serde_json::Value::as_str) {
            Some("progress") => {
                saw_progress = true;
                last_progress_line = line.to_string();
                anyhow::ensure!(
                    v.get("schema").and_then(serde_json::Value::as_str) == Some("wrkr.ndjson.v1"),
                    "expected a progress json object with schema=wrkr.ndjson.v1\nstdout:\n{}\nstderr:\n{}",
                    stdout,
                    stderr
                );
                anyhow::ensure!(
                    v.get("elapsedSeconds").is_some(),
                    "expected a progress json object with `elapsedSeconds` key\nstdout:\n{}\nstderr:\n{}",
                    stdout,
                    stderr
                );
            }
            Some("summary") => {
                saw_summary = true;
                anyhow::ensure!(
                    v.get("schema").and_then(serde_json::Value::as_str) == Some("wrkr.ndjson.v1"),
                    "expected a summary json object with schema=wrkr.ndjson.v1\nstdout:\n{}\nstderr:\n{}",
                    stdout,
                    stderr
                );
                anyhow::ensure!(
                    v.get("totals").is_some(),
                    "expected a summary json object with `totals` key\nstdout:\n{}\nstderr:\n{}",
                    stdout,
                    stderr
                );
                anyhow::ensure!(
                    v.pointer("/thresholds/violations").is_some(),
                    "expected a summary json object with `thresholds.violations`\nstdout:\n{}\nstderr:\n{}",
                    stdout,
                    stderr
                );
            }
            _ => {}
        }
    }

    anyhow::ensure!(
        saw_progress,
        "expected at least one progress json line\nstdout:\n{}\nstderr:\n{}",
        stdout,
        stderr
    );
    anyhow::ensure!(
        saw_summary,
        "expected a final summary json line\nstdout:\n{}\nstderr:\n{}",
        stdout,
        stderr
    );
    anyhow::ensure!(
        !last_progress_line.is_empty(),
        "expected to capture a progress json line\nstdout:\n{}\nstderr:\n{}",
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
