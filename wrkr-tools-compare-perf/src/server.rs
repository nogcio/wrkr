use anyhow::{Context, Result, bail};
use std::io::{BufRead, BufReader, Read};
use std::path::Path;
use std::process::{Child, Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};

#[derive(Debug, Clone)]
pub(crate) struct ServerTargets {
    pub(crate) base_url: String,
    pub(crate) grpc_target: String,
}

pub(crate) struct TestServer {
    child: Child,
    reader: BufReader<std::process::ChildStdout>,
    stderr: BufReader<std::process::ChildStderr>,
    started_at: Instant,
    base_url: Option<String>,
    grpc_target: Option<String>,
}

impl TestServer {
    pub(crate) fn start(root: &Path, server_bin: &Path) -> Result<Self> {
        println!("Starting wrkr-testserver...");

        let mut cmd = Command::new(server_bin);
        cmd.current_dir(root)
            .args(["--bind", "127.0.0.1:0"])
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());

        let mut child = cmd.spawn().context("spawn wrkr-testserver")?;

        let stdout = child.stdout.take().context("take wrkr-testserver stdout")?;
        let stderr = child.stderr.take().context("take wrkr-testserver stderr")?;

        Ok(Self {
            child,
            reader: BufReader::new(stdout),
            stderr: BufReader::new(stderr),
            started_at: Instant::now(),
            base_url: None,
            grpc_target: None,
        })
    }

    pub(crate) fn wait_for_targets(&mut self, timeout: Duration) -> Result<ServerTargets> {
        let deadline = Instant::now() + timeout;

        loop {
            if let (Some(base_url), Some(grpc_target)) = (&self.base_url, &self.grpc_target) {
                println!("Server: {base_url}");
                println!("gRPC: {grpc_target}");
                return Ok(ServerTargets {
                    base_url: base_url.clone(),
                    grpc_target: grpc_target.clone(),
                });
            }

            if Instant::now() > deadline {
                let mut stderr = String::new();
                self.stderr.read_to_string(&mut stderr).ok();
                bail!(
                    "timed out waiting for BASE_URL/GRPC_TARGET from testserver (elapsed={:?})\nstderr:\n{stderr}",
                    self.started_at.elapsed()
                );
            }

            let mut line = String::new();
            let n = self.reader.read_line(&mut line).unwrap_or(0);
            if n == 0 {
                let status = self.child.try_wait().context("poll testserver status")?;
                if let Some(status) = status {
                    let mut stderr = String::new();
                    self.stderr.read_to_string(&mut stderr).ok();
                    bail!("testserver exited early: {status}\nstderr:\n{stderr}");
                }

                thread::sleep(Duration::from_millis(10));
                continue;
            }

            let line = line.trim_end();
            if let Some(rest) = line.strip_prefix("BASE_URL=") {
                self.base_url = Some(rest.to_string());
            }
            if let Some(rest) = line.strip_prefix("GRPC_TARGET=") {
                self.grpc_target = Some(rest.to_string());
            }
        }
    }

    pub(crate) fn shutdown(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

impl Drop for TestServer {
    fn drop(&mut self) {
        self.shutdown();
    }
}
