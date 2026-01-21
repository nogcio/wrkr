use crate::cli::Cli;
use crate::parse::{parse_k6_grpc_rps, parse_k6_http_rps, parse_wrk_rps, parse_wrkr_rps};
use crate::report::{print_grpc_summary, print_http_summary};
use crate::runner::{print_invocation, run_with_rss_sampling};
use crate::tools::ToolPaths;
use crate::types::{Rps, RunResult};
use anyhow::Result;
use std::path::Path;
use std::process::{Command, Stdio};

#[derive(Debug, Clone, Copy)]
pub(crate) struct HttpCaseScripts {
    pub(crate) wrk: &'static str,
    pub(crate) wrkr: &'static str,
    pub(crate) k6: &'static str,
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct HttpCase {
    pub(crate) title: &'static str,
    pub(crate) scripts: HttpCaseScripts,
    pub(crate) ratio_ok_wrkr_over_wrk: f64,
    pub(crate) ratio_ok_wrkr_over_k6: f64,
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct GrpcCaseScripts {
    pub(crate) wrkr: &'static str,
    pub(crate) k6: &'static str,
}

#[derive(Debug, Default)]
pub(crate) struct CaseRun {
    pub(crate) failures: u32,
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct HttpCaseOutcome {
    pub(crate) failures: u32,
    pub(crate) wrk_rps: Option<Rps>,
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct GrpcCaseOutcome {
    pub(crate) failures: u32,
    pub(crate) wrkr_rps: Rps,
}

pub(crate) fn run_http_case(
    root: &Path,
    tools: &ToolPaths,
    base_url: &str,
    cli: &Cli,
    case: HttpCase,
) -> Result<HttpCaseOutcome> {
    let title = case.title;
    let scripts = case.scripts;
    let ratio_ok_wrkr_over_wrk = case.ratio_ok_wrkr_over_wrk;
    let ratio_ok_wrkr_over_k6 = case.ratio_ok_wrkr_over_k6;

    println!("\n====================");
    println!("CASE: {title}");
    println!("====================");

    let mut out = CaseRun::default();

    let wrk_res = if let Some(wrk) = &tools.wrk {
        println!("== wrk ==");
        let mut cmd = Command::new(wrk);
        cmd.current_dir(root);
        cmd.args([
            format!("-t{}", cli.wrk_threads),
            format!("-c{}", cli.wrk_connections),
            format!("-d{}", cli.duration),
            "-s".to_string(),
            root.join(scripts.wrk).display().to_string(),
            base_url.to_string(),
        ])
        .stdin(Stdio::null());

        print_invocation("wrk", &cmd, Some(root), &[]);
        Some(run_with_rss_sampling(cmd)?)
    } else {
        println!("== wrk == (skipped: not installed)");
        None
    };

    println!("== wrkr ==");
    let wrkr_res = {
        let vus = cli.wrkr_vus.to_string();
        let env = format!("BASE_URL={base_url}");
        let mut cmd = Command::new(&tools.wrkr);
        cmd.current_dir(root)
            .arg("run")
            .arg(scripts.wrkr)
            .arg("--duration")
            .arg(&cli.duration)
            .arg("--vus")
            .arg(vus)
            .arg("--env")
            .arg(env)
            .stdin(Stdio::null());

        print_invocation("wrkr", &cmd, Some(root), &[]);
        run_with_rss_sampling(cmd)?
    };

    let k6_res = if let Some(k6) = &tools.k6 {
        println!("== k6 ==");
        let vus = cli.k6_vus.unwrap_or(cli.wrkr_vus);
        let mut cmd = Command::new(k6);
        cmd.current_dir(root)
            .env("BASE_URL", base_url)
            .arg("run")
            .arg("--vus")
            .arg(vus.to_string())
            .arg("--duration")
            .arg(&cli.duration)
            .arg(root.join(scripts.k6))
            .stdin(Stdio::null());

        print_invocation("k6", &cmd, Some(root), &[("BASE_URL", base_url)]);
        Some(run_with_rss_sampling(cmd)?)
    } else {
        println!("== k6 == (skipped: not installed)");
        None
    };

    let wrkr_rps = parse_wrkr_rps(&wrkr_res)?;

    let wrk_rps = wrk_res
        .as_ref()
        .map(|r| parse_wrk_rps(&r.stdout))
        .transpose()?;

    let k6_rps = k6_res.as_ref().map(parse_k6_http_rps).transpose()?;

    print_http_summary(
        wrk_res.as_ref(),
        &wrkr_res,
        k6_res.as_ref(),
        wrk_rps,
        wrkr_rps,
        k6_rps,
    );

    if let Some(wrk_rps) = wrk_rps {
        let ratio_actual = wrkr_rps.0 / wrk_rps.0;
        if is_too_slow(wrkr_rps, wrk_rps, ratio_ok_wrkr_over_wrk, true) {
            println!(
                "FAIL: wrkr is too slow vs wrk (ratio_ok={ratio_ok_wrkr_over_wrk}, ratio_actual={ratio_actual:.3})"
            );
            out.failures += 1;
        } else {
            println!("PASS: wrkr/wrk >= {ratio_ok_wrkr_over_wrk} (ratio_actual={ratio_actual:.3})");
        }
    }

    if let Some(k6_rps) = k6_rps {
        let ratio_actual = wrkr_rps.0 / k6_rps.0;
        if is_too_slow(wrkr_rps, k6_rps, ratio_ok_wrkr_over_k6, false) {
            println!(
                "FAIL: wrkr is too slow vs k6 (ratio_ok={ratio_ok_wrkr_over_k6}, ratio_actual={ratio_actual:.3})"
            );
            out.failures += 1;
        } else {
            println!("PASS: wrkr/k6 > {ratio_ok_wrkr_over_k6} (ratio_actual={ratio_actual:.3})");
        }
    }

    Ok(HttpCaseOutcome {
        failures: out.failures,
        wrk_rps,
    })
}

pub(crate) fn run_grpc_case(
    title: &str,
    root: &Path,
    tools: &ToolPaths,
    grpc_target: &str,
    cli: &Cli,
    scripts: GrpcCaseScripts,
    ratio_ok_wrkr_over_k6: f64,
) -> Result<GrpcCaseOutcome> {
    println!("\n====================");
    println!("CASE: {title}");
    println!("====================");

    let mut out = CaseRun::default();

    println!("== wrkr ==");
    let wrkr_res = {
        let vus = cli.wrkr_vus.to_string();
        let env = format!("GRPC_TARGET={grpc_target}");
        let mut cmd = Command::new(&tools.wrkr);
        cmd.current_dir(root)
            .arg("run")
            .arg(scripts.wrkr)
            .arg("--duration")
            .arg(&cli.duration)
            .arg("--vus")
            .arg(vus)
            .arg("--env")
            .arg(env)
            .stdin(Stdio::null());

        print_invocation("wrkr", &cmd, Some(root), &[]);
        run_with_rss_sampling(cmd)?
    };

    let wrkr_rps = parse_wrkr_rps(&wrkr_res)?;

    let k6_res = if let Some(k6) = &tools.k6 {
        println!("== k6 ==");
        let vus = cli.k6_vus.unwrap_or(cli.wrkr_vus);
        let mut cmd = Command::new(k6);
        cmd.current_dir(root)
            .env("GRPC_TARGET", grpc_target)
            .arg("run")
            .arg("--vus")
            .arg(vus.to_string())
            .arg("--duration")
            .arg(&cli.duration)
            .arg(root.join(scripts.k6))
            .stdin(Stdio::null());

        print_invocation("k6", &cmd, Some(root), &[("GRPC_TARGET", grpc_target)]);
        Some(run_with_rss_sampling(cmd)?)
    } else {
        println!("== k6 == (skipped: not installed)");
        None
    };

    let k6_rps = k6_res.as_ref().map(parse_k6_grpc_rps).transpose()?;

    print_grpc_summary(&wrkr_res, k6_res.as_ref(), wrkr_rps, k6_rps);

    if let Some(k6_rps) = k6_rps {
        let ratio_actual = wrkr_rps.0 / k6_rps.0;
        if is_too_slow(wrkr_rps, k6_rps, ratio_ok_wrkr_over_k6, false) {
            println!(
                "FAIL: wrkr is too slow vs k6 (ratio_ok={ratio_ok_wrkr_over_k6}, ratio_actual={ratio_actual:.3})"
            );
            out.failures += 1;
        } else {
            println!("PASS: wrkr/k6 > {ratio_ok_wrkr_over_k6} (ratio_actual={ratio_actual:.3})");
        }
    }

    Ok(GrpcCaseOutcome {
        failures: out.failures,
        wrkr_rps,
    })
}

pub(crate) fn is_too_slow(wrkr: Rps, other: Rps, ratio: f64, inclusive: bool) -> bool {
    if inclusive {
        // wrkr < other * ratio => too slow
        wrkr.0 + f64::EPSILON < other.0 * ratio
    } else {
        // wrkr <= other * ratio => too slow
        wrkr.0 <= other.0 * ratio
    }
}

#[allow(dead_code)]
fn _debug_dump(name: &str, res: &RunResult) {
    println!("--- {name}: status={} ---", res.status);
    println!("stdout:\n{}", res.stdout);
    if !res.stderr.is_empty() {
        println!("stderr:\n{}", res.stderr);
    }
}
