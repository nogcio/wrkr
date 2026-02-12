use anyhow::Context as _;
use wrkr_testserver::TestServer;

#[path = "support/helpers.rs"]
mod support;

#[tokio::test]
async fn e2e_wrk_runs_for_1s_and_sends_requests() -> anyhow::Result<()> {
    let server = TestServer::start().await.context("start test server")?;
    let url = format!("{}/plaintext", server.base_url());

    let output = support::run_wrkr_output(
        [
            "--duration".to_string(),
            "1s".to_string(),
            "--output".to_string(),
            "json".to_string(),
            "-H".to_string(),
            "X-Test: 1".to_string(),
            "-T".to_string(),
            "2s".to_string(),
            url,
        ],
        std::iter::empty::<(&'static str, &'static str)>(),
        std::time::Duration::from_secs(15),
    )
    .await
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

    let mut saw_summary = false;

    for line in stdout.lines().filter(|l| !l.trim().is_empty()) {
        let v: serde_json::Value = serde_json::from_str(line)
            .with_context(|| format!("failed to parse json line: {line}"))?;

        if let Some("summary") = v.get("kind").and_then(serde_json::Value::as_str) {
            saw_summary = true;
            anyhow::ensure!(
                v.get("schema").and_then(serde_json::Value::as_str) == Some("wrkr.ndjson.v1"),
                "expected a summary json object with schema=wrkr.ndjson.v1\nstdout:\n{}\nstderr:\n{}",
                stdout,
                stderr
            );
        }
    }

    anyhow::ensure!(
        saw_summary,
        "expected a final summary json line\nstdout:\n{}\nstderr:\n{}",
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

#[tokio::test]
async fn e2e_wrk_compat_runs_without_subcommand() -> anyhow::Result<()> {
    let server = TestServer::start().await.context("start test server")?;
    let url = format!("{}/plaintext", server.base_url());

    let output = support::run_wrkr_output(
        [
            "-c".to_string(),
            "10".to_string(),
            "-d".to_string(),
            "1s".to_string(),
            "--output".to_string(),
            "json".to_string(),
            url,
        ],
        std::iter::empty::<(&'static str, &'static str)>(),
        std::time::Duration::from_secs(15),
    )
    .await
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

    anyhow::ensure!(
        stdout.lines().any(|l| l.contains("\"kind\":\"summary\"")),
        "expected a final summary json line\nstdout:\n{}\nstderr:\n{}",
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
