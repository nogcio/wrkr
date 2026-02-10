use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::Duration;

use anyhow::{Context, Result, bail};
use clap::Args as ClapArgs;

mod parse;
mod server;
mod tools;

#[derive(Debug, ClapArgs)]
pub struct Args {
    /// Repo root (defaults to current working directory)
    #[arg(long, env = "WRKR_ROOT")]
    pub root: Option<PathBuf>,

    /// Duration for each case (e.g. 5s)
    #[arg(long, env = "DURATION", default_value = "5s")]
    pub duration: String,

    /// Build required binaries before running
    #[arg(long, default_value_t = true)]
    pub build: bool,

    /// Build with -C target-cpu=native (machine-specific best perf)
    #[arg(long, env = "NATIVE", default_value_t = true)]
    pub native: bool,

    /// wrkr VUs
    #[arg(long, env = "WRKR_VUS", default_value_t = 256)]
    pub wrkr_vus: u32,

    /// k6 VUs (defaults to wrkr_vus)
    #[arg(long, env = "K6_VUS")]
    pub k6_vus: Option<u32>,

    /// wrk threads
    #[arg(long, env = "WRK_THREADS", default_value_t = 8)]
    pub wrk_threads: u32,

    /// wrk connections
    #[arg(long, env = "WRK_CONNECTIONS", default_value_t = 256)]
    pub wrk_connections: u32,

    /// Gate: wrkr_rps must be >= wrk_rps * ratio (GET /hello)
    #[arg(long, env = "RATIO_OK", default_value_t = 0.90)]
    pub ratio_ok_get_hello: f64,

    /// Gate: wrkr_rps must be >= wrk_rps * ratio (POST json)
    #[arg(long, env = "RATIO_OK_POST_JSON", default_value_t = 0.90)]
    pub ratio_ok_post_json: f64,

    /// Gate: wrkr_rps must be >= wrk_rps * ratio (wfb json aggregate)
    #[arg(long, env = "RATIO_OK_WFB_JSON_AGGREGATE", default_value_t = 0.90)]
    pub ratio_ok_wfb_json_aggregate: f64,

    /// Gate: wrkr_rps must be > k6_rps * ratio (HTTP)
    #[arg(long, env = "RATIO_OK_WRKR_OVER_K6", default_value_t = 1.40)]
    pub ratio_ok_wrkr_over_k6: f64,

    /// Gate: wrkr gRPC rps must be > k6 grpc rps * ratio
    #[arg(long, env = "RATIO_OK_GRPC_WRKR_OVER_K6", default_value_t = 2.00)]
    pub ratio_ok_grpc_wrkr_over_k6: f64,

    /// Gate: wrkr wfb gRPC rps must be > k6 grpc rps * ratio
    #[arg(
        long,
        env = "RATIO_OK_WFB_GRPC_AGGREGATE_WRKR_OVER_K6",
        default_value_t = 1.20
    )]
    pub ratio_ok_wfb_grpc_aggregate_wrkr_over_k6: f64,

    /// Optional cross-protocol gate: wrkr gRPC RPS must be >= wrk GET /hello RPS * ratio
    #[arg(
        long,
        env = "RATIO_OK_GRPC_WRKR_OVER_WRK_HELLO",
        default_value_t = 0.70
    )]
    pub ratio_ok_grpc_wrkr_over_wrk_hello: f64,

    /// If set, missing wrk is a hard error; otherwise wrk comparisons are skipped
    #[arg(long, default_value_t = false)]
    pub require_wrk: bool,

    /// If set, missing k6 is a hard error; otherwise k6 comparisons are skipped
    #[arg(long, default_value_t = false)]
    pub require_k6: bool,
}

#[derive(Debug, Clone)]
struct CaseHttp {
    title: &'static str,
    wrk_script: &'static str,
    wrkr_script: &'static str,
    k6_script: &'static str,
    ratio_ok_wrkr_over_wrk: f64,
    ratio_ok_wrkr_over_k6: f64,
}

#[derive(Debug, Clone)]
struct CaseGrpc {
    title: &'static str,
    wrkr_script: &'static str,
    k6_script: &'static str,
    ratio_ok_wrkr_over_k6: f64,
}

pub fn run(args: Args) -> Result<()> {
    let root = tools::resolve_repo_root(args.root)?;

    validate_positive_ratio("ratio_ok_get_hello", args.ratio_ok_get_hello)?;
    validate_positive_ratio("ratio_ok_post_json", args.ratio_ok_post_json)?;
    validate_positive_ratio(
        "ratio_ok_wfb_json_aggregate",
        args.ratio_ok_wfb_json_aggregate,
    )?;
    validate_positive_ratio("ratio_ok_wrkr_over_k6", args.ratio_ok_wrkr_over_k6)?;
    validate_positive_ratio(
        "ratio_ok_grpc_wrkr_over_k6",
        args.ratio_ok_grpc_wrkr_over_k6,
    )?;
    validate_positive_ratio(
        "ratio_ok_wfb_grpc_aggregate_wrkr_over_k6",
        args.ratio_ok_wfb_grpc_aggregate_wrkr_over_k6,
    )?;
    validate_positive_ratio(
        "ratio_ok_grpc_wrkr_over_wrk_hello",
        args.ratio_ok_grpc_wrkr_over_wrk_hello,
    )?;

    let duration_seconds = parse::parse_duration_seconds(&args.duration)
        .with_context(|| format!("invalid --duration {:?}", args.duration))?;

    if args.wrkr_vus == 0 {
        bail!("--wrkr-vus must be >= 1");
    }
    if args.wrk_threads == 0 {
        bail!("--wrk-threads must be >= 1");
    }
    if args.wrk_connections == 0 {
        bail!("--wrk-connections must be >= 1");
    }

    let k6_vus = args.k6_vus.unwrap_or(args.wrkr_vus);

    if args.build {
        build_binaries(&root, args.native)?;
    }

    let tool_paths = tools::detect_tools(
        &root,
        tools::ToolRequirements {
            require_wrk: args.require_wrk,
            require_k6: args.require_k6,
        },
    )?;

    let http_cases = [
        CaseHttp {
            title: "GET /hello",
            wrk_script: "tools/perf/wrk_hello.lua",
            wrkr_script: "tools/perf/wrkr_hello.lua",
            k6_script: "tools/perf/k6_hello.js",
            ratio_ok_wrkr_over_wrk: args.ratio_ok_get_hello,
            ratio_ok_wrkr_over_k6: args.ratio_ok_wrkr_over_k6,
        },
        CaseHttp {
            title: "POST /echo (json + checks)",
            wrk_script: "tools/perf/wrk_post_json.lua",
            wrkr_script: "tools/perf/wrkr_post_json.lua",
            k6_script: "tools/perf/k6_post_json.js",
            ratio_ok_wrkr_over_wrk: args.ratio_ok_post_json,
            ratio_ok_wrkr_over_k6: args.ratio_ok_wrkr_over_k6,
        },
        CaseHttp {
            title: "POST /analytics/aggregate (wfb json + checks)",
            wrk_script: "tools/perf/wrk_wfb_json_aggregate.lua",
            wrkr_script: "tools/perf/wrkr_wfb_json_aggregate.lua",
            k6_script: "tools/perf/k6_wfb_json_aggregate.js",
            ratio_ok_wrkr_over_wrk: args.ratio_ok_wfb_json_aggregate,
            ratio_ok_wrkr_over_k6: args.ratio_ok_wrkr_over_k6,
        },
    ];

    let grpc_cases = [
        CaseGrpc {
            title: "gRPC Echo (plaintext)",
            wrkr_script: "tools/perf/wrkr_grpc_plaintext.lua",
            k6_script: "tools/perf/k6_grpc_plaintext.js",
            ratio_ok_wrkr_over_k6: args.ratio_ok_grpc_wrkr_over_k6,
        },
        CaseGrpc {
            title: "gRPC AggregateOrders (wfb)",
            wrkr_script: "tools/perf/wfb_grpc_aggregate.lua",
            k6_script: "tools/perf/k6_wfb_grpc_aggregate.js",
            ratio_ok_wrkr_over_k6: args.ratio_ok_wfb_grpc_aggregate_wrkr_over_k6,
        },
    ];

    println!("==> perf-gate");
    println!("root={}", root.display());
    println!("duration={} ({}s)", args.duration, duration_seconds);
    println!(
        "wrkr_vus={} k6_vus={} wrk_threads={} wrk_connections={}",
        args.wrkr_vus, k6_vus, args.wrk_threads, args.wrk_connections
    );

    let mut failures: Vec<String> = Vec::new();
    let mut summaries: Vec<String> = Vec::new();

    let mut hello_wrk_rps: Option<f64> = None;
    let mut first_grpc_wrkr_rps: Option<f64> = None;

    let mut server = server::TestServer::start(&root, &tool_paths.wrkr_testserver)?;
    let targets = server.wait_for_targets(Duration::from_secs(10))?;
    println!("http_url={}", targets.http_url);
    println!("grpc_url={}", targets.grpc_url);

    // HTTP suite
    for (idx, case) in http_cases.iter().enumerate() {
        println!("\n==> HTTP: {}", case.title);
        ensure_exists(&root, case.wrkr_script)?;
        ensure_exists(&root, case.wrk_script)?;
        ensure_exists(&root, case.k6_script)?;

        // wrk
        let mut wrk_rps: Option<f64> = None;
        if let Some(wrk_bin) = tool_paths.wrk.as_ref() {
            let output = run_wrk(
                &root,
                wrk_bin,
                case.wrk_script,
                &args.duration,
                args.wrk_threads,
                args.wrk_connections,
                &targets.http_url,
            )?;
            let parsed = parse::parse_wrk(&output.stdout)?;
            if !parsed.errors.is_empty() {
                for e in parsed.errors {
                    failures.push(format!("HTTP {}: wrk correctness: {e}", case.title));
                }
            }
            wrk_rps = Some(parsed.rps);
        } else {
            println!("wrk: SKIP (not installed)");
        }

        if idx == 0 {
            hello_wrk_rps = wrk_rps;
        }

        // wrkr
        let wrkr_output = run_wrkr(
            &root,
            &tool_paths.wrkr,
            case.wrkr_script,
            &args.duration,
            args.wrkr_vus,
            &targets.http_url,
            false,
        )?;
        let wrkr_rps = parse::parse_wrkr_rps(
            &wrkr_output.stdout,
            &wrkr_output.stderr,
            Some(duration_seconds),
        )?;

        // k6
        let mut k6_rps: Option<f64> = None;
        let mut k6_failed_rate: Option<f64> = None;
        let mut k6_warn_failed: u32 = 0;
        if let Some(k6_bin) = tool_paths.k6.as_ref() {
            let k6_output = run_k6_http(
                &root,
                k6_bin,
                case.k6_script,
                &args.duration,
                k6_vus,
                &targets.http_url,
            )?;
            k6_rps = Some(parse::parse_k6_http_rps(
                &k6_output.stdout,
                &k6_output.stderr,
            )?);
            k6_failed_rate = parse::parse_k6_req_failed_rate(
                "http_req_failed",
                &k6_output.stdout,
                &k6_output.stderr,
            );
            k6_warn_failed =
                parse::count_k6_request_failed_warnings(&k6_output.stdout, &k6_output.stderr);
        } else {
            println!("k6: SKIP (not installed)");
        }

        summaries.push(format!(
            "HTTP {}: rps wrk={} wrkr={:.3} k6={} ",
            case.title,
            opt_fmt(wrk_rps),
            wrkr_rps,
            opt_fmt(k6_rps)
        ));

        // correctness gates
        if wrkr_output.status != 0 {
            failures.push(format!(
                "HTTP {}: wrkr exited with {}",
                case.title, wrkr_output.status
            ));
        }

        if let Some(rate) = k6_failed_rate
            && rate > 0.0
        {
            failures.push(format!(
                "HTTP {}: k6 http_req_failed={:.4}",
                case.title, rate
            ));
        }
        if k6_warn_failed > 0 {
            failures.push(format!(
                "HTTP {}: k6 request failed warnings={}",
                case.title, k6_warn_failed
            ));
        }

        // perf gates
        if let Some(wrk_rps) = wrk_rps
            && wrkr_rps + f64::EPSILON < (wrk_rps * case.ratio_ok_wrkr_over_wrk)
        {
            failures.push(format!(
                "HTTP {}: wrkr too slow vs wrk (ratio_ok={}, ratio_actual={:.3})",
                case.title,
                case.ratio_ok_wrkr_over_wrk,
                wrkr_rps / wrk_rps
            ));
        }

        if let Some(k6_rps) = k6_rps
            && wrkr_rps <= (k6_rps * case.ratio_ok_wrkr_over_k6)
        {
            failures.push(format!(
                "HTTP {}: wrkr too slow vs k6 (ratio_ok={}, ratio_actual={:.3})",
                case.title,
                case.ratio_ok_wrkr_over_k6,
                wrkr_rps / k6_rps
            ));
        }
    }

    // gRPC suite
    for (idx, case) in grpc_cases.iter().enumerate() {
        println!("\n==> gRPC: {}", case.title);
        ensure_exists(&root, case.wrkr_script)?;
        ensure_exists(&root, case.k6_script)?;

        let wrkr_output = run_wrkr(
            &root,
            &tool_paths.wrkr,
            case.wrkr_script,
            &args.duration,
            args.wrkr_vus,
            &targets.grpc_url,
            true,
        )?;
        let wrkr_rps = parse::parse_wrkr_rps(
            &wrkr_output.stdout,
            &wrkr_output.stderr,
            Some(duration_seconds),
        )?;

        if idx == 0 {
            first_grpc_wrkr_rps = Some(wrkr_rps);
        }

        let mut k6_rps: Option<f64> = None;
        let mut k6_failed_rate: Option<f64> = None;
        let mut k6_warn_failed: u32 = 0;
        if let Some(k6_bin) = tool_paths.k6.as_ref() {
            let k6_output = run_k6_grpc(
                &root,
                k6_bin,
                case.k6_script,
                &args.duration,
                k6_vus,
                &targets.grpc_url,
            )?;
            k6_rps = Some(parse::parse_k6_grpc_rps(
                &k6_output.stdout,
                &k6_output.stderr,
            )?);
            k6_failed_rate = parse::parse_k6_req_failed_rate(
                "grpc_req_failed",
                &k6_output.stdout,
                &k6_output.stderr,
            )
            .or_else(|| {
                parse::parse_k6_req_failed_rate(
                    "http_req_failed",
                    &k6_output.stdout,
                    &k6_output.stderr,
                )
            });
            k6_warn_failed =
                parse::count_k6_request_failed_warnings(&k6_output.stdout, &k6_output.stderr);
        } else {
            println!("k6: SKIP (not installed)");
        }

        summaries.push(format!(
            "gRPC {}: rps wrkr={:.3} k6={} ",
            case.title,
            wrkr_rps,
            opt_fmt(k6_rps)
        ));

        if wrkr_output.status != 0 {
            failures.push(format!(
                "gRPC {}: wrkr exited with {}",
                case.title, wrkr_output.status
            ));
        }
        if let Some(rate) = k6_failed_rate
            && rate > 0.0
        {
            failures.push(format!("gRPC {}: k6 req_failed={:.4}", case.title, rate));
        }
        if k6_warn_failed > 0 {
            failures.push(format!(
                "gRPC {}: k6 request failed warnings={}",
                case.title, k6_warn_failed
            ));
        }

        if let Some(k6_rps) = k6_rps
            && wrkr_rps <= (k6_rps * case.ratio_ok_wrkr_over_k6)
        {
            failures.push(format!(
                "gRPC {}: wrkr too slow vs k6 (ratio_ok={}, ratio_actual={:.3})",
                case.title,
                case.ratio_ok_wrkr_over_k6,
                wrkr_rps / k6_rps
            ));
        }
    }

    // Cross-protocol gate: wrkr gRPC vs wrk hello
    if let (Some(grpc_wrkr), Some(wrk_hello)) = (first_grpc_wrkr_rps, hello_wrk_rps) {
        if grpc_wrkr + f64::EPSILON < (wrk_hello * args.ratio_ok_grpc_wrkr_over_wrk_hello) {
            failures.push(format!(
                "cross-protocol: wrkr grpc too slow vs wrk hello (ratio_ok={}, ratio_actual={:.3})",
                args.ratio_ok_grpc_wrkr_over_wrk_hello,
                grpc_wrkr / wrk_hello
            ));
        }
    } else {
        println!("cross-protocol gate: SKIP (missing wrk or gRPC metric)");
    }

    // Ensure server is shut down.
    server.shutdown();

    println!("\n==> SUMMARY");
    for s in &summaries {
        println!("- {s}");
    }

    if failures.is_empty() {
        println!("\nOVERALL: PASS");
        return Ok(());
    }

    println!("\nOVERALL: FAIL ({} issue(s))", failures.len());
    for f in &failures {
        println!("- {f}");
    }

    bail!("perf gate failed")
}

fn validate_positive_ratio(name: &str, value: f64) -> Result<()> {
    if value > 0.0 {
        Ok(())
    } else {
        bail!("{name} must be > 0, got {value}")
    }
}

fn ensure_exists(root: &Path, rel: &str) -> Result<()> {
    let path = root.join(rel);
    if path.exists() {
        Ok(())
    } else {
        bail!("missing script: {}", path.display())
    }
}

#[derive(Debug)]
struct Captured {
    status: i32,
    stdout: String,
    stderr: String,
}

fn build_binaries(root: &Path, native: bool) -> Result<()> {
    println!("==> build release binaries");

    let mut env: BTreeMap<String, String> = BTreeMap::new();
    if native {
        env.insert("RUSTFLAGS".to_string(), "-C target-cpu=native".to_string());
    }

    run_checked(
        root,
        {
            let mut cmd = Command::new("cargo");
            cmd.arg("build")
                .arg("--release")
                .arg("-p")
                .arg("wrkr-testserver")
                .arg("--bin")
                .arg("wrkr-testserver");
            cmd
        },
        &env,
    )?;

    run_checked(
        root,
        {
            let mut cmd = Command::new("cargo");
            cmd.arg("build").arg("--release").arg("--bin").arg("wrkr");
            cmd
        },
        &env,
    )?;

    Ok(())
}

fn run_wrk(
    root: &Path,
    wrk_bin: &Path,
    script_rel: &str,
    duration: &str,
    threads: u32,
    conns: u32,
    base_url: &str,
) -> Result<Captured> {
    println!("wrk");
    let mut cmd = Command::new(wrk_bin);
    cmd.arg(format!("-t{threads}"))
        .arg(format!("-c{conns}"))
        .arg(format!("-d{duration}"))
        .arg("-s")
        .arg(root.join(script_rel))
        .arg(base_url)
        .current_dir(root)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());

    capture(cmd)
}

fn run_wrkr(
    root: &Path,
    wrkr_bin: &Path,
    script_rel: &str,
    duration: &str,
    vus: u32,
    base_url: &str,
    is_grpc: bool,
) -> Result<Captured> {
    println!("wrkr");

    let mut env = no_proxy_env();
    env.insert("BASE_URL".to_string(), base_url.to_string());

    let mut cmd = Command::new(wrkr_bin);
    cmd.arg("run")
        .arg(script_rel)
        .arg("--output")
        .arg("json")
        .arg("--duration")
        .arg(duration)
        .arg("--vus")
        .arg(vus.to_string())
        .arg("--env")
        .arg(format!("BASE_URL={base_url}"))
        .current_dir(root)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());

    // A tiny hint in logs so it is clear which target is used.
    if is_grpc {
        println!("target(grpc)={base_url}");
    } else {
        println!("target(http)={base_url}");
    }

    capture_with_env(cmd, &env)
}

fn run_k6_http(
    root: &Path,
    k6_bin: &Path,
    script_rel: &str,
    duration: &str,
    vus: u32,
    base_url: &str,
) -> Result<Captured> {
    println!("k6");

    let mut env = no_proxy_env();
    env.insert("BASE_URL".to_string(), base_url.to_string());

    let mut cmd = Command::new(k6_bin);
    cmd.arg("run")
        .arg("--vus")
        .arg(vus.to_string())
        .arg("--duration")
        .arg(duration)
        .arg(root.join(script_rel))
        .current_dir(root)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());

    capture_with_env(cmd, &env)
}

fn run_k6_grpc(
    root: &Path,
    k6_bin: &Path,
    script_rel: &str,
    duration: &str,
    vus: u32,
    grpc_url: &str,
) -> Result<Captured> {
    println!("k6");

    let mut env = no_proxy_env();
    env.insert("BASE_URL".to_string(), grpc_url.to_string());

    let mut cmd = Command::new(k6_bin);
    cmd.arg("run")
        .arg("--vus")
        .arg(vus.to_string())
        .arg("--duration")
        .arg(duration)
        .arg(root.join(script_rel))
        .current_dir(root)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());

    capture_with_env(cmd, &env)
}

fn run_checked(root: &Path, mut cmd: Command, env: &BTreeMap<String, String>) -> Result<()> {
    cmd.current_dir(root)
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit());
    for (k, v) in env {
        cmd.env(k, v);
    }
    let status = cmd.status().context("failed to run command")?;
    if status.success() {
        Ok(())
    } else {
        bail!("command failed: {status}")
    }
}

fn capture(mut cmd: Command) -> Result<Captured> {
    let output = cmd.output().context("failed to run command")?;
    Ok(Captured {
        status: output.status.code().unwrap_or(1),
        stdout: String::from_utf8_lossy(&output.stdout).to_string(),
        stderr: String::from_utf8_lossy(&output.stderr).to_string(),
    })
}

fn capture_with_env(mut cmd: Command, env: &BTreeMap<String, String>) -> Result<Captured> {
    for (k, v) in env {
        cmd.env(k, v);
    }
    capture(cmd)
}

fn no_proxy_env() -> BTreeMap<String, String> {
    let add = ["127.0.0.1", "localhost", "::1"];

    fn merge(existing: Option<&str>, add: &[&str]) -> String {
        let mut parts: Vec<String> = Vec::new();
        if let Some(v) = existing {
            for p in v.split(',') {
                let s = p.trim();
                if !s.is_empty() {
                    parts.push(s.to_string());
                }
            }
        }
        for &p in add {
            if !parts.iter().any(|x| x == p) {
                parts.push(p.to_string());
            }
        }
        parts.join(",")
    }

    let existing = std::env::var("NO_PROXY")
        .ok()
        .filter(|s| !s.trim().is_empty());
    let existing2 = std::env::var("no_proxy")
        .ok()
        .filter(|s| !s.trim().is_empty());
    let merged = merge(existing.as_deref().or(existing2.as_deref()), &add);

    let mut env = BTreeMap::new();
    env.insert("NO_PROXY".to_string(), merged.clone());
    env.insert("no_proxy".to_string(), merged);
    env
}

fn opt_fmt(v: Option<f64>) -> String {
    match v {
        None => "-".to_string(),
        Some(x) => format!("{x:.3}"),
    }
}
