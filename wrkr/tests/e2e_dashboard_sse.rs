use std::path::Path;
use std::process::Stdio;
use std::time::Duration;

use anyhow::Context as _;
use tokio::io::{AsyncBufReadExt as _, AsyncReadExt as _, AsyncWriteExt as _};
use tokio::net::TcpStream;
use tokio::process::Command;
use tokio::time::timeout;
use wrkr_testserver::TestServer;

struct ChildGuard {
    child: tokio::process::Child,
}

impl ChildGuard {
    async fn wait(mut self) -> std::io::Result<std::process::ExitStatus> {
        self.child.wait().await
    }
}

impl Drop for ChildGuard {
    fn drop(&mut self) {
        let _ = self.child.start_kill();
    }
}

fn parse_socket_addr(url: &str) -> anyhow::Result<std::net::SocketAddr> {
    let url = url.trim();
    let url = url
        .strip_prefix("http://")
        .with_context(|| format!("expected http:// URL, got: {url}"))?;
    let addr: std::net::SocketAddr = url.parse().context("parse dashboard addr")?;
    Ok(addr)
}

#[tokio::test]
async fn e2e_dashboard_sse_streams_ticks() -> anyhow::Result<()> {
    let server = TestServer::start().await.context("start test server")?;
    let base_url = server.base_url().to_string();

    let script_path = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/scripts/hello_world.lua");
    let exe = env!("CARGO_BIN_EXE_wrkr");

    let mut child = Command::new(exe)
        .arg("run")
        .arg(&script_path)
        .arg("--duration")
        .arg("3s")
        .arg("--output")
        .arg("json")
        .arg("--dashboard")
        .env("BASE_URL", &base_url)
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
        .context("spawn wrkr")?;

    let stderr = child.stderr.take().context("take stderr")?;
    let mut stderr = tokio::io::BufReader::new(stderr).lines();

    let url_line = timeout(Duration::from_secs(5), async {
        loop {
            let line = stderr.next_line().await?;
            let Some(line) = line else {
                anyhow::bail!("wrkr exited before printing dashboard url");
            };
            if line.starts_with("dashboard: http://") {
                return Ok::<_, anyhow::Error>(line);
            }
        }
    })
    .await
    .context("timeout waiting for dashboard url")??;

    let url = url_line
        .strip_prefix("dashboard: ")
        .context("unexpected dashboard url line")?
        .trim();

    let addr = parse_socket_addr(url)?;

    // Connect to SSE endpoint and ensure we see at least one tick.
    let mut stream = TcpStream::connect(addr)
        .await
        .context("connect to dashboard")?;
    let req = format!(
        "GET /events HTTP/1.1\r\nHost: {addr}\r\nAccept: text/event-stream\r\nConnection: close\r\n\r\n"
    );
    stream.write_all(req.as_bytes()).await?;

    let mut buf = Vec::new();
    let saw = timeout(Duration::from_secs(6), async {
        loop {
            let mut chunk = [0u8; 4096];
            let n = stream.read(&mut chunk).await?;
            if n == 0 {
                anyhow::bail!("dashboard SSE closed before tick");
            }
            buf.extend_from_slice(&chunk[..n]);
            let text = String::from_utf8_lossy(&buf);
            if text.contains("event: snapshot") && text.contains("event: tick") {
                return Ok::<_, anyhow::Error>(());
            }
        }
    })
    .await;

    // Ensure the process is waited (and doesn't leak) even if the SSE check fails.
    let status = ChildGuard { child }.wait().await?;

    let server_seen = server.stats().requests_total();
    server.shutdown().await;

    anyhow::ensure!(status.success(), "wrkr exited with {status}");
    saw.context("timeout waiting for snapshot + tick")??;
    anyhow::ensure!(server_seen > 0, "expected server to see requests");

    Ok(())
}
