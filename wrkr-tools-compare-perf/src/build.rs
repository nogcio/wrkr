use anyhow::{Context, Result, bail};
use std::path::Path;
use std::process::Command;

pub(crate) fn build_binaries(root: &Path, native: bool) -> Result<()> {
    println!("Building release binaries...");

    let rustflags = if native { "-C target-cpu=native" } else { "" };

    let status = Command::new("cargo")
        .current_dir(root)
        .env("RUSTFLAGS", rustflags)
        .args([
            "build",
            "--release",
            "-p",
            "wrkr-testserver",
            "--bin",
            "wrkr-testserver",
        ])
        .status()
        .context("build wrkr-testserver")?;

    if !status.success() {
        bail!("failed to build wrkr-testserver");
    }

    let status = Command::new("cargo")
        .current_dir(root)
        .env("RUSTFLAGS", rustflags)
        .args(["build", "--release", "--bin", "wrkr"])
        .status()
        .context("build wrkr")?;

    if !status.success() {
        bail!("failed to build wrkr");
    }

    Ok(())
}
