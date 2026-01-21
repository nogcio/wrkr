use crate::build::build_binaries;
use crate::cases::{
    GrpcCaseScripts, HttpCase, HttpCaseScripts, is_too_slow, run_grpc_case, run_http_case,
};
use crate::cli::Cli;
use crate::server::TestServer;
use crate::tools::ToolPaths;
use anyhow::{Context, Result, bail};
use std::time::Duration;

pub fn run(cli: Cli) -> Result<()> {
    let root = cli
        .root
        .clone()
        .unwrap_or(std::env::current_dir().context("get cwd")?);

    let ratio_ok_grpc_wrkr_over_k6 = cli.ratio_ok_grpc_wrkr_over_k6;

    if cli.build {
        build_binaries(&root, cli.native)?;
    }

    let tools = ToolPaths::detect(&root, cli.require_wrk, cli.require_k6)?;

    let mut server = TestServer::start(&root, &tools.wrkr_testserver)?;
    let targets = server.wait_for_targets(Duration::from_secs(5))?;

    let mut failures = 0u32;

    println!(
        "duration={} | wrk: threads={} conns={} | wrkr: vus={} | k6: vus={}",
        cli.duration,
        cli.wrk_threads,
        cli.wrk_connections,
        cli.wrkr_vus,
        cli.k6_vus.unwrap_or(cli.wrkr_vus)
    );

    let hello = run_http_case(
        &root,
        &tools,
        &targets.base_url,
        &cli,
        HttpCase {
            title: "GET /hello",
            scripts: HttpCaseScripts {
                wrk: "tools/perf/wrk_hello.lua",
                wrkr: "tools/perf/wrkr_hello.lua",
                k6: "tools/perf/k6_hello.js",
            },
            ratio_ok_wrkr_over_wrk: cli.ratio_ok_get_hello,
            ratio_ok_wrkr_over_k6: cli.ratio_ok_wrkr_over_k6,
        },
    )?;
    failures += hello.failures;

    let _post_json = run_http_case(
        &root,
        &tools,
        &targets.base_url,
        &cli,
        HttpCase {
            title: "POST /echo (json + checks)",
            scripts: HttpCaseScripts {
                wrk: "tools/perf/wrk_post_json.lua",
                wrkr: "tools/perf/wrkr_post_json.lua",
                k6: "tools/perf/k6_post_json.js",
            },
            ratio_ok_wrkr_over_wrk: cli.ratio_ok_post_json,
            ratio_ok_wrkr_over_k6: cli.ratio_ok_wrkr_over_k6,
        },
    )?;
    failures += _post_json.failures;

    let grpc = run_grpc_case(
        "gRPC Echo (plaintext)",
        &root,
        &tools,
        &targets.grpc_target,
        &cli,
        GrpcCaseScripts {
            wrkr: "tools/perf/wrkr_grpc_plaintext.lua",
            k6: "tools/perf/k6_grpc_plaintext.js",
        },
        ratio_ok_grpc_wrkr_over_k6,
    )?;
    failures += grpc.failures;

    // Cross-protocol comparison: wrkr gRPC vs wrk GET /hello.
    if let Some(wrk_hello) = hello.wrk_rps {
        let ratio_actual = grpc.wrkr_rps.0 / wrk_hello.0;
        let ratio_ok = cli.ratio_ok_grpc_wrkr_over_wrk_hello;
        if is_too_slow(grpc.wrkr_rps, wrk_hello, ratio_ok, true) {
            println!(
                "FAIL: wrkr grpc is too slow vs wrk hello (ratio_ok={ratio_ok}, ratio_actual={ratio_actual:.3})"
            );
            failures += 1;
        } else {
            println!("PASS: wrkr-grpc/wrk-hello >= {ratio_ok} (ratio_actual={ratio_actual:.3})");
        }
    } else {
        println!(
            "INFO: wrkr-grpc/wrk-hello ratio skipped (wrk not installed or hello wrk skipped)"
        );
    }

    // Explicit shutdown for nicer logs; Drop also does it.
    server.shutdown();

    if failures > 0 {
        bail!("OVERALL: FAIL ({failures} failing case(s))");
    }

    println!("\nOVERALL: PASS");
    Ok(())
}
