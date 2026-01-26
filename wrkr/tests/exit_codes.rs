use std::path::Path;
use std::process::Command;

use anyhow::Context as _;
use wrkr_testserver::TestServer;

fn status_code(status: std::process::ExitStatus) -> i32 {
    status.code().unwrap_or(-1)
}

#[test]
fn invalid_flags_exit_30() -> anyhow::Result<()> {
    let exe = env!("CARGO_BIN_EXE_wrkr");

    let out = Command::new(exe)
        .arg("run")
        .arg("./does-not-matter.lua")
        .arg("--duration")
        .arg("10x")
        .output()
        .context("run wrkr binary")?;

    anyhow::ensure!(
        status_code(out.status) == 30,
        "expected exit code 30, got {}\nstdout:\n{}\nstderr:\n{}",
        status_code(out.status),
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );

    Ok(())
}

#[tokio::test]
async fn checks_failed_exit_10() -> anyhow::Result<()> {
    let server = TestServer::start().await.context("start test server")?;
    let base_url = server.base_url().to_string();

    let script_path = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/scripts/checks_fail.lua");
    let exe = env!("CARGO_BIN_EXE_wrkr");

    let out = tokio::task::spawn_blocking(move || {
        Command::new(exe)
            .arg("run")
            .arg(&script_path)
            .arg("--iterations")
            .arg("1")
            .arg("--output")
            .arg("json")
            .env("BASE_URL", &base_url)
            .output()
    })
    .await
    .context("spawn_blocking join")?
    .context("run wrkr binary")?;

    server.shutdown().await;

    anyhow::ensure!(
        status_code(out.status) == 10,
        "expected exit code 10, got {}\nstdout:\n{}\nstderr:\n{}",
        status_code(out.status),
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );

    Ok(())
}

#[tokio::test]
async fn thresholds_failed_exit_11() -> anyhow::Result<()> {
    let script_path =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/scripts/thresholds_fail.lua");
    let exe = env!("CARGO_BIN_EXE_wrkr");

    let out = tokio::task::spawn_blocking(move || {
        Command::new(exe)
            .arg("run")
            .arg(&script_path)
            .arg("--iterations")
            .arg("1")
            .arg("--output")
            .arg("json")
            .output()
    })
    .await
    .context("spawn_blocking join")?
    .context("run wrkr binary")?;

    anyhow::ensure!(
        status_code(out.status) == 11,
        "expected exit code 11, got {}\nstdout:\n{}\nstderr:\n{}",
        status_code(out.status),
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );

    Ok(())
}
