use std::env;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};

#[derive(Debug, Clone, Copy)]
pub struct ToolRequirements {
    pub require_wrk: bool,
    pub require_k6: bool,
}

#[derive(Debug, Clone)]
pub struct ToolPaths {
    pub wrk: Option<PathBuf>,
    pub k6: Option<PathBuf>,
    pub wrkr: PathBuf,
    pub wrkr_testserver: PathBuf,
}

pub fn resolve_repo_root(root: Option<PathBuf>) -> Result<PathBuf> {
    match root {
        Some(p) => Ok(p),
        None => Ok(env::current_dir().context("failed to get current_dir")?),
    }
}

pub fn detect_tools(root: &Path, req: ToolRequirements) -> Result<ToolPaths> {
    let root = root
        .canonicalize()
        .with_context(|| format!("failed to canonicalize root {}", root.display()))?;

    let wrkr = root.join("target").join("release").join(exe_name("wrkr"));
    if !wrkr.exists() {
        bail!(
            "Missing binary: {} (build first or pass --build)",
            wrkr.display()
        );
    }

    let wrkr_testserver = root
        .join("target")
        .join("release")
        .join(exe_name("wrkr-testserver"));
    if !wrkr_testserver.exists() {
        bail!(
            "Missing binary: {} (build first or pass --build)",
            wrkr_testserver.display()
        );
    }

    let wrk = which("wrk");
    if req.require_wrk && wrk.is_none() {
        bail!("Missing required command: wrk (not found on PATH)");
    }

    let k6 = which("k6");
    if req.require_k6 && k6.is_none() {
        bail!("Missing required command: k6 (not found on PATH)");
    }

    Ok(ToolPaths {
        wrk,
        k6,
        wrkr,
        wrkr_testserver,
    })
}

fn exe_name(base: &str) -> String {
    if cfg!(windows) && !base.to_ascii_lowercase().ends_with(".exe") {
        format!("{base}.exe")
    } else {
        base.to_string()
    }
}

fn which(cmd: &str) -> Option<PathBuf> {
    let path = env::var_os("PATH")?;
    for dir in env::split_paths(&path) {
        let full = dir.join(cmd);
        if full.is_file() {
            return Some(full);
        }
        if cfg!(windows) {
            let full_exe = dir.join(format!("{cmd}.exe"));
            if full_exe.is_file() {
                return Some(full_exe);
            }
        }
    }
    None
}
