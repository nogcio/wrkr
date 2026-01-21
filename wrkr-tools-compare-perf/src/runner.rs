use crate::types::RunResult;
use anyhow::{Context, Result};
use std::io::Read;
use std::path::Path;
use std::process::{Command, Stdio};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::thread;
use std::time::Duration;
use sysinfo::{Pid, ProcessRefreshKind, ProcessesToUpdate, RefreshKind, System};

pub(crate) fn print_invocation(
    label: &str,
    cmd: &Command,
    cwd: Option<&Path>,
    env: &[(&str, &str)],
) {
    if let Some(cwd) = cwd {
        println!("{label}: cwd={}", cwd.display());
    }
    if !env.is_empty() {
        let pairs = env
            .iter()
            .map(|(k, v)| format!("{k}={}", quote_for_display(v)))
            .collect::<Vec<_>>()
            .join(" ");
        println!("{label}: env {pairs}");
    }
    println!("{label}: {}", command_to_string(cmd));
}

fn command_to_string(cmd: &Command) -> String {
    let prog = cmd.get_program().to_string_lossy();
    let mut out = String::new();
    out.push_str(&quote_for_display(&prog));
    for arg in cmd.get_args() {
        out.push(' ');
        out.push_str(&quote_for_display(&arg.to_string_lossy()));
    }
    out
}

fn quote_for_display(s: &str) -> String {
    // Not a shell-accurate escaper; just makes spaces/specials unambiguous in logs.
    let needs_quotes = s
        .chars()
        .any(|c| c.is_whitespace() || matches!(c, '"' | '\\'));
    if !needs_quotes {
        return s.to_string();
    }

    let mut out = String::with_capacity(s.len() + 2);
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            _ => out.push(c),
        }
    }
    out.push('"');
    out
}

pub(crate) fn run_with_rss_sampling(mut cmd: Command) -> Result<RunResult> {
    cmd.stdout(Stdio::piped()).stderr(Stdio::piped());

    let mut child = cmd.spawn().context("spawn child")?;
    let pid = child.id();

    let stdout = child.stdout.take().context("take stdout")?;
    let stderr = child.stderr.take().context("take stderr")?;

    let stop = Arc::new(AtomicBool::new(false));
    let peak = Arc::new(AtomicU64::new(0));

    let stop_sampler = Arc::clone(&stop);
    let peak_sampler = Arc::clone(&peak);

    let sampler = thread::spawn(move || {
        let pid = Pid::from_u32(pid);
        let refresh = RefreshKind::nothing().with_processes(ProcessRefreshKind::everything());
        let mut sys = System::new_with_specifics(refresh);

        while !stop_sampler.load(Ordering::Relaxed) {
            sys.refresh_processes_specifics(
                ProcessesToUpdate::Some(&[pid]),
                true,
                ProcessRefreshKind::everything(),
            );
            if let Some(p) = sys.process(pid) {
                // Normalize to bytes. sysinfo's units differ across platforms:
                // - macOS: bytes
                // - most others: KiB
                let rss_bytes = {
                    let mem = p.memory();
                    #[cfg(target_os = "macos")]
                    {
                        mem
                    }

                    #[cfg(not(target_os = "macos"))]
                    {
                        mem * 1024
                    }
                };
                update_max(&peak_sampler, rss_bytes);
            }

            thread::sleep(Duration::from_millis(50));
        }
    });

    let out_handle = thread::spawn(move || read_to_string(stdout));
    let err_handle = thread::spawn(move || read_to_string(stderr));

    let status = child.wait().context("wait child")?;
    stop.store(true, Ordering::Relaxed);
    let _ = sampler.join();

    let stdout = out_handle
        .join()
        .ok()
        .unwrap_or_else(|| Ok(String::new()))?;
    let stderr = err_handle
        .join()
        .ok()
        .unwrap_or_else(|| Ok(String::new()))?;

    Ok(RunResult {
        status,
        stdout,
        stderr,
        peak_rss_bytes: peak.load(Ordering::Relaxed),
    })
}

fn update_max(cur: &AtomicU64, candidate: u64) {
    let mut existing = cur.load(Ordering::Relaxed);
    while candidate > existing {
        match cur.compare_exchange(existing, candidate, Ordering::Relaxed, Ordering::Relaxed) {
            Ok(_) => return,
            Err(v) => existing = v,
        }
    }
}

fn read_to_string<R: Read>(mut r: R) -> Result<String> {
    let mut s = String::new();
    r.read_to_string(&mut s).context("read stream")?;
    Ok(s)
}
