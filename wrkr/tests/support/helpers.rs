use std::ffi::OsStr;
use std::process::{Output, Stdio};
use std::time::Duration;

use anyhow::Context as _;
use tokio::io::AsyncReadExt as _;
use tokio::process::Command;

pub(crate) async fn run_wrkr_output(
    args: impl IntoIterator<Item = impl AsRef<OsStr>>,
    env: impl IntoIterator<Item = (impl AsRef<OsStr>, impl AsRef<OsStr>)>,
    timeout_dur: Duration,
) -> anyhow::Result<Output> {
    let exe = env!("CARGO_BIN_EXE_wrkr");

    let mut cmd = Command::new(exe);
    cmd.args(args)
        .envs(env)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());

    let mut child = cmd.spawn().context("spawn wrkr")?;

    let mut child_stdout = child.stdout.take().context("take child stdout")?;
    let mut child_stderr = child.stderr.take().context("take child stderr")?;

    let stdout_task = tokio::spawn(async move {
        let mut buf = Vec::new();
        child_stdout
            .read_to_end(&mut buf)
            .await
            .context("read stdout")?;
        Ok::<_, anyhow::Error>(buf)
    });

    let stderr_task = tokio::spawn(async move {
        let mut buf = Vec::new();
        child_stderr
            .read_to_end(&mut buf)
            .await
            .context("read stderr")?;
        Ok::<_, anyhow::Error>(buf)
    });

    let status = tokio::select! {
        status = child.wait() => status.context("wait wrkr")?,
        _ = tokio::time::sleep(timeout_dur) => {
            let _ = child.start_kill();
            let _ = child.wait().await;
            anyhow::bail!("wrkr timed out after {timeout_dur:?}");
        }
    };

    let stdout = stdout_task.await.context("join stdout task")??;
    let stderr = stderr_task.await.context("join stderr task")??;

    Ok(Output {
        status,
        stdout,
        stderr,
    })
}
