use anyhow::{Result, bail};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone)]
pub(crate) struct ToolPaths {
    pub(crate) wrk: Option<PathBuf>,
    pub(crate) k6: Option<PathBuf>,
    pub(crate) wrkr: PathBuf,
    pub(crate) wrkr_testserver: PathBuf,
}

impl ToolPaths {
    pub(crate) fn detect(root: &Path, require_wrk: bool, require_k6: bool) -> Result<Self> {
        let wrkr = root.join("target/release/wrkr");
        if !wrkr.exists() {
            bail!("missing binary: {wrkr:?} (build first or pass --build)");
        }

        let wrkr_testserver = root.join("target/release/wrkr-testserver");
        if !wrkr_testserver.exists() {
            bail!("missing binary: {wrkr_testserver:?} (build first or pass --build)");
        }

        let wrk = which("wrk");
        if require_wrk && wrk.is_none() {
            bail!("missing required command: wrk");
        }

        let k6 = which("k6");
        if require_k6 && k6.is_none() {
            bail!("missing required command: k6");
        }

        Ok(Self {
            wrk,
            k6,
            wrkr,
            wrkr_testserver,
        })
    }
}

fn which(cmd: &str) -> Option<PathBuf> {
    let path = std::env::var_os("PATH")?;
    std::env::split_paths(&path).find_map(|dir| {
        let candidate = dir.join(cmd);
        if candidate.is_file() {
            return Some(candidate);
        }

        #[cfg(windows)]
        {
            let candidate = dir.join(format!("{cmd}.exe"));
            if candidate.is_file() {
                return Some(candidate);
            }
        }

        None
    })
}
