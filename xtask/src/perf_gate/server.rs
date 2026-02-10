use std::io::{BufRead, BufReader};
use std::path::Path;
use std::process::{Child, Command, Stdio};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

use anyhow::{Context, Result, bail};

#[derive(Debug, Clone)]
pub struct ServerTargets {
    pub http_url: String,
    pub grpc_url: String,
}

#[derive(Debug)]
struct State {
    http_url: Option<String>,
    grpc_url: Option<String>,
    stderr_tail: Vec<String>,
}

pub struct TestServer {
    child: Child,
    state: Arc<Mutex<State>>,
}

impl TestServer {
    pub fn start(root: &Path, server_bin: &Path) -> Result<Self> {
        if !server_bin.exists() {
            bail!("wrkr-testserver binary not found: {}", server_bin.display());
        }

        println!("==> start wrkr-testserver");

        let mut child = Command::new(server_bin)
            .arg("--bind")
            .arg("127.0.0.1:0")
            .current_dir(root)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .with_context(|| format!("failed to spawn {}", server_bin.display()))?;

        let stdout = child
            .stdout
            .take()
            .context("failed to take testserver stdout")?;
        let stderr = child
            .stderr
            .take()
            .context("failed to take testserver stderr")?;

        let state = Arc::new(Mutex::new(State {
            http_url: None,
            grpc_url: None,
            stderr_tail: Vec::new(),
        }));

        // stdout reader
        {
            let state = Arc::clone(&state);
            thread::spawn(move || {
                let reader = BufReader::new(stdout);
                for line in reader.lines().map_while(Result::ok) {
                    let line = line.trim_end().to_string();
                    if let Ok(mut st) = state.lock() {
                        if let Some(rest) = line.strip_prefix("HTTP_URL=") {
                            st.http_url = Some(rest.trim().to_string());
                        } else if let Some(rest) = line.strip_prefix("GRPC_URL=") {
                            st.grpc_url = Some(rest.trim().to_string());
                        }
                    }
                }
            });
        }

        // stderr reader (tail)
        {
            let state = Arc::clone(&state);
            thread::spawn(move || {
                let reader = BufReader::new(stderr);
                for line in reader.lines().map_while(Result::ok) {
                    let line = line.trim_end().to_string();
                    if line.is_empty() {
                        continue;
                    }
                    if let Ok(mut st) = state.lock() {
                        st.stderr_tail.push(line);
                        const MAX: usize = 200;
                        if st.stderr_tail.len() > MAX {
                            let drain = st.stderr_tail.len() - MAX;
                            st.stderr_tail.drain(0..drain);
                        }
                    }
                }
            });
        }

        Ok(Self { child, state })
    }

    pub fn wait_for_targets(&mut self, timeout: Duration) -> Result<ServerTargets> {
        let deadline = Instant::now() + timeout;
        loop {
            if let Ok(st) = self.state.lock()
                && let (Some(http_url), Some(grpc_url)) = (st.http_url.clone(), st.grpc_url.clone())
            {
                return Ok(ServerTargets { http_url, grpc_url });
            }

            if let Some(status) = self.child.try_wait().context("failed to poll testserver")? {
                let tail = self.stderr_tail(50);
                bail!("testserver exited early: {status}\n--- stderr (tail) ---\n{tail}");
            }

            if Instant::now() >= deadline {
                let tail = self.stderr_tail(50);
                bail!("timed out waiting for HTTP_URL/GRPC_URL\n--- stderr (tail) ---\n{tail}");
            }

            thread::sleep(Duration::from_millis(10));
        }
    }

    pub fn shutdown(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }

    fn stderr_tail(&self, n: usize) -> String {
        match self.state.lock() {
            Ok(st) => {
                let start = st.stderr_tail.len().saturating_sub(n);
                st.stderr_tail[start..].join("\n")
            }
            Err(_) => String::new(),
        }
    }
}

impl Drop for TestServer {
    fn drop(&mut self) {
        self.shutdown();
    }
}
