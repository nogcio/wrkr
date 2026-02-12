use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::Context as _;

use crate::cli::{RunArgs, RunConfigArgs, RunOutputArgs, WrkModeArgs};
use crate::exit_codes::ExitCode;
use crate::run;
use crate::run_error::RunError;

struct TempScriptPath {
    path: std::path::PathBuf,
}

impl Drop for TempScriptPath {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.path);
    }
}

pub async fn run_wrk_mode(
    wrk: WrkModeArgs,
    mut cfg: RunConfigArgs,
    out: RunOutputArgs,
) -> Result<ExitCode, RunError> {
    let Some(url) = wrk.url.as_deref() else {
        return Err(RunError::InvalidInput(anyhow::anyhow!(
            "missing URL (usage: wrkr [options] <url>)"
        )));
    };

    // If a script was provided (`-s`), treat it as a `wrkr` script
    // and default BASE_URL to the provided URL (unless already set).
    if let Some(script) = wrk.script {
        if cfg.vus.is_none() {
            cfg.vus = wrk.threads;
        }

        if !cfg.env.iter().any(|e| e.starts_with("BASE_URL=")) {
            cfg.env.push(format!("BASE_URL={url}"));
        }

        return run::run(
            RunArgs {
                script,
                scenario: None,
            },
            cfg,
            out,
        )
        .await;
    }

    if cfg.vus.is_none() {
        cfg.vus = wrk.threads;
    }

    inject_wrk_env_overrides(url, &wrk, &mut cfg).map_err(RunError::InvalidInput)?;

    let script = include_str!("../assets/wrk_universal.lua");

    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let file_name = format!("wrkr_url_mode_{}_{}.lua", std::process::id(), nanos);

    let mut path = std::env::temp_dir();
    path.push(file_name);

    std::fs::write(&path, script)
        .with_context(|| format!("failed to write temp script: {}", path.display()))
        .map_err(RunError::RuntimeError)?;

    let _guard = TempScriptPath { path: path.clone() };

    run::run(
        RunArgs {
            script: path,
            scenario: None,
        },
        cfg,
        out,
    )
    .await
}

fn inject_wrk_env_overrides(
    url: &str,
    wrk: &WrkModeArgs,
    cfg: &mut RunConfigArgs,
) -> Result<(), anyhow::Error> {
    let method = wrk.method.trim();
    if method.is_empty() {
        anyhow::bail!("--method cannot be empty");
    }

    let mut headers_kv: Vec<(String, String)> = Vec::new();
    for raw in &wrk.header {
        let (k, v) = parse_header(raw).with_context(|| format!("invalid --header: {raw}"))?;
        headers_kv.push((k, v));
    }

    let headers_json = serde_json::to_string(
        &headers_kv
            .into_iter()
            .map(|(k, v)| serde_json::json!({"k": k, "v": v}))
            .collect::<Vec<_>>(),
    )
    .context("failed to encode headers")?;

    cfg.env.push(format!("WRKR_REQUEST_URL={url}"));
    cfg.env.push(format!("WRKR_REQUEST_METHOD={method}"));
    cfg.env
        .push(format!("WRKR_REQUEST_HEADERS_JSON={headers_json}"));

    if let Some(timeout) = wrk.timeout.as_deref() {
        cfg.env.push(format!("WRKR_REQUEST_TIMEOUT={timeout}"));
    }
    if let Some(name) = wrk.name.as_deref() {
        cfg.env.push(format!("WRKR_REQUEST_NAME={name}"));
    }

    match wrk.body.as_deref() {
        Some(body) => {
            cfg.env.push("WRKR_REQUEST_BODY_PRESENT=1".to_string());
            cfg.env.push(format!("WRKR_REQUEST_BODY={body}"));
        }
        None => {
            cfg.env.push("WRKR_REQUEST_BODY_PRESENT=0".to_string());
        }
    }

    Ok(())
}

fn parse_header(raw: &str) -> Result<(String, String), anyhow::Error> {
    let s = raw.trim();
    if s.is_empty() {
        anyhow::bail!("empty header");
    }

    if let Some((k, v)) = s.split_once(':') {
        let key = k.trim();
        let value = v.trim();
        if key.is_empty() {
            anyhow::bail!("empty header key");
        }
        return Ok((key.to_string(), value.to_string()));
    }

    if let Some((k, v)) = s.split_once('=') {
        let key = k.trim();
        let value = v.trim();
        if key.is_empty() {
            anyhow::bail!("empty header key");
        }
        return Ok((key.to_string(), value.to_string()));
    }

    anyhow::bail!("expected KEY:VALUE or KEY=VALUE");
}
