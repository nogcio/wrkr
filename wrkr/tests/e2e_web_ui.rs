use std::path::Path;
use std::process::Stdio;

use anyhow::Context as _;
use futures_util::StreamExt as _;
use tokio::io::{AsyncBufReadExt as _, BufReader};
use tokio_tungstenite::tungstenite::Message;
use url::Url;
use wrkr_testserver::TestServer;

#[tokio::test]
async fn e2e_web_ui_serves_ws_snapshot_and_updates() -> anyhow::Result<()> {
    let server = TestServer::start().await.context("start test server")?;
    let base_url = server.base_url().to_string();

    let script_path = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/scripts/hello_world.lua");
    let exe = env!("CARGO_BIN_EXE_wrkr");

    let mut child = tokio::process::Command::new(exe)
        .arg("run")
        .arg(&script_path)
        .arg("--duration")
        .arg("2s")
        .arg("--output")
        .arg("json")
        .arg("--dashboard")
        .arg("--dashboard-bind")
        .arg("127.0.0.1:0")
        .env("BASE_URL", &base_url)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .context("spawn wrkr")?;

    let stderr = child.stderr.take().context("missing stderr")?;
    let mut stderr_lines = BufReader::new(stderr).lines();

    let web_url = tokio::time::timeout(std::time::Duration::from_secs(2), async {
        while let Some(line) = stderr_lines.next_line().await? {
            if let Some(v) = line.strip_prefix("dashboard=") {
                return Ok::<_, anyhow::Error>(v.trim().to_string());
            }
        }
        anyhow::bail!("dashboard url not found on stderr");
    })
    .await
    .context("timed out waiting for dashboard url")??;

    let mut ws_url = Url::parse(&web_url).context("parse dashboard url")?;
    ws_url
        .set_scheme("ws")
        .map_err(|_| anyhow::anyhow!("failed to set ws scheme"))?;
    ws_url.set_path("/ws");

    let (mut ws, _resp) = tokio_tungstenite::connect_async(ws_url.to_string())
        .await
        .context("connect ws")?;

    // First message should be a snapshot (possibly empty).
    let snapshot = tokio::time::timeout(std::time::Duration::from_secs(2), ws.next())
        .await
        .context("timed out waiting for ws snapshot")?
        .context("ws stream ended")?
        .context("ws message error")?;

    let Message::Text(snapshot_text) = snapshot else {
        anyhow::bail!("expected text snapshot message, got: {snapshot:?}");
    };
    let snapshot_v: serde_json::Value =
        serde_json::from_str(&snapshot_text).context("parse snapshot json")?;
    anyhow::ensure!(
        snapshot_v.get("type") == Some(&serde_json::Value::String("snapshot".to_string())),
        "expected snapshot message, got: {snapshot_text}"
    );

    // Next, expect at least one update within a short time window.
    let update = tokio::time::timeout(std::time::Duration::from_secs(3), async {
        loop {
            let msg = ws.next().await.context("ws stream ended")??;
            if let Message::Text(text) = msg {
                let v: serde_json::Value = serde_json::from_str(&text)?;
                if v.get("type") == Some(&serde_json::Value::String("update".to_string())) {
                    return Ok::<_, anyhow::Error>(text);
                }
            }
        }
    })
    .await
    .context("timed out waiting for ws update")??;

    anyhow::ensure!(
        update.contains(r#""type":"update""#),
        "expected update json, got: {update}"
    );

    let status = tokio::time::timeout(std::time::Duration::from_secs(5), child.wait())
        .await
        .context("timed out waiting for wrkr to exit")?
        .context("wait wrkr")?;

    let server_seen = server.stats().requests_total();
    server.shutdown().await;

    anyhow::ensure!(status.success(), "wrkr exited with {status}");
    anyhow::ensure!(server_seen > 0, "expected server to see requests");

    Ok(())
}
