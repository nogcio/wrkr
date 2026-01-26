use std::path::Path;
use std::process::Command;
use std::time::Instant;

use anyhow::Context as _;
use serde::Deserialize;
use wrkr_testserver::TestServer;

#[derive(Debug, Clone, Copy)]
struct ProgressSample {
    elapsed_seconds: f64,
    interval_seconds: f64,
    requests_per_sec: f64,
    total_requests: u64,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Totals {
    requests_total: u64,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ProgressMetrics {
    requests_per_sec: f64,
    req_per_sec_avg: f64,
    total_requests: u64,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ProgressLine {
    schema: String,
    elapsed_seconds: f64,
    interval_seconds: f64,
    metrics: ProgressMetrics,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SummaryLine {
    schema: String,
    totals: Totals,
}

#[derive(Debug, Deserialize)]
#[serde(tag = "kind")]
enum JsonLine {
    #[serde(rename = "progress")]
    Progress(ProgressLine),

    #[serde(rename = "summary")]
    Summary(SummaryLine),
}

#[tokio::test]
async fn e2e_stats_rps_matches_server_observed_rps() -> anyhow::Result<()> {
    let server = TestServer::start().await.context("start test server")?;
    let base_url = server.base_url().to_string();

    let script_path = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/scripts/hello_world.lua");
    let exe = env!("CARGO_BIN_EXE_wrkr");

    // Keep it short but stable enough to get at least a few progress ticks.
    let duration = "4s";
    let vus = "4";

    let start = Instant::now();
    let output = tokio::task::spawn_blocking(move || {
        Command::new(exe)
            .arg("run")
            .arg(&script_path)
            .arg("--duration")
            .arg(duration)
            .arg("--vus")
            .arg(vus)
            .arg("--output")
            .arg("json")
            .env("BASE_URL", &base_url)
            .output()
    })
    .await
    .context("spawn_blocking join")?
    .context("run wrkr binary")?;
    let wall = start.elapsed();

    let server_seen = server.stats().requests_total();
    server.shutdown().await;

    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();

    anyhow::ensure!(
        output.status.success(),
        "wrkr exited with {}\nstdout:\n{}\nstderr:\n{}",
        output.status,
        stdout,
        stderr
    );

    let mut progress_samples: Vec<ProgressSample> = Vec::new();
    let mut best_progress: Option<(f64, f64, f64, u64)> = None; // (elapsed_seconds, requests_per_sec, req_per_sec_avg, total_requests)
    let mut summary_requests_total: Option<u64> = None;

    for line in stdout.lines().filter(|l| !l.trim().is_empty()) {
        let parsed: JsonLine = serde_json::from_str(line)
            .with_context(|| format!("failed to parse json line: {line}"))?;

        match parsed {
            JsonLine::Progress(p) => {
                anyhow::ensure!(
                    p.schema == "wrkr.ndjson.v1",
                    "unexpected schema in progress line: {}",
                    p.schema
                );

                if p.elapsed_seconds <= 0.0 {
                    continue;
                }

                progress_samples.push(ProgressSample {
                    elapsed_seconds: p.elapsed_seconds,
                    interval_seconds: p.interval_seconds,
                    requests_per_sec: p.metrics.requests_per_sec,
                    total_requests: p.metrics.total_requests,
                });

                match best_progress {
                    None => {
                        best_progress = Some((
                            p.elapsed_seconds,
                            p.metrics.requests_per_sec,
                            p.metrics.req_per_sec_avg,
                            p.metrics.total_requests,
                        ));
                    }
                    Some((best_elapsed, ..)) if p.elapsed_seconds > best_elapsed => {
                        best_progress = Some((
                            p.elapsed_seconds,
                            p.metrics.requests_per_sec,
                            p.metrics.req_per_sec_avg,
                            p.metrics.total_requests,
                        ));
                    }
                    Some(_) => {}
                }
            }
            JsonLine::Summary(s) => {
                anyhow::ensure!(
                    s.schema == "wrkr.ndjson.v1",
                    "unexpected schema in summary line: {}",
                    s.schema
                );
                summary_requests_total = Some(s.totals.requests_total);
            }
        }
    }

    let (elapsed_seconds, wrkr_rps_now, wrkr_rps_avg, wrkr_total_requests) = best_progress
        .with_context(|| {
            format!(
                "expected at least one progress json line\nstdout:\n{stdout}\nstderr:\n{stderr}"
            )
        })?;

    let summary_total_requests = summary_requests_total.with_context(|| {
        format!("expected a final summary json line\nstdout:\n{stdout}\nstderr:\n{stderr}")
    })?;

    anyhow::ensure!(
        summary_total_requests >= wrkr_total_requests,
        "expected final summary total_requests >= last progress total_requests\nsummary_total_requests={summary_total_requests}\nprogress_total_requests={wrkr_total_requests}\nstdout:\n{stdout}\nstderr:\n{stderr}"
    );

    anyhow::ensure!(
        server_seen > 0,
        "expected server to see requests\nserver_seen={server_seen}\nstdout:\n{stdout}\nstderr:\n{stderr}"
    );

    // Sanity: totals should match what the server observed.
    // Allow a small tolerance to avoid flakes from in-flight requests at shutdown.
    let totals_delta = server_seen.abs_diff(summary_total_requests);
    anyhow::ensure!(
        totals_delta <= 10,
        "request totals mismatch\nwrkr_summary_requests_total={summary_total_requests}\nserver_seen={server_seen}\ndelta={totals_delta}\nprogress_total_requests={wrkr_total_requests}\nelapsed_seconds={elapsed_seconds}\nwall_elapsed={wall:?}\nstdout:\n{stdout}\nstderr:\n{stderr}"
    );

    // Internal consistency: integrating the live per-second rate should approximately match
    // the reported live `total_requests` at the last progress tick.
    progress_samples.sort_by(|a, b| a.elapsed_seconds.total_cmp(&b.elapsed_seconds));
    anyhow::ensure!(
        !progress_samples.is_empty(),
        "expected progress samples\nstdout:\n{stdout}\nstderr:\n{stderr}"
    );

    let mut integrated_requests = 0.0_f64;
    let mut integrated_secs = 0.0_f64;
    let mut prev_elapsed = 0.0_f64;
    let mut prev_total = 0_u64;

    for s in &progress_samples {
        anyhow::ensure!(
            s.elapsed_seconds > prev_elapsed,
            "expected monotonic elapsedSeconds in progress\nprev_elapsed={prev_elapsed}\nelapsed_seconds={}\nstdout:\n{stdout}\nstderr:\n{stderr}",
            s.elapsed_seconds
        );
        anyhow::ensure!(
            s.total_requests >= prev_total,
            "expected monotonic total_requests in progress\nprev_total={prev_total}\ntotal_requests={}\nstdout:\n{stdout}\nstderr:\n{stderr}",
            s.total_requests
        );

        let delta_secs = s.elapsed_seconds - prev_elapsed;
        // intervalSeconds comes from wrkr itself.
        // Expect these to be close enough to detect bugs without being flaky.
        anyhow::ensure!(
            (s.interval_seconds - delta_secs).abs() <= 0.5,
            "unexpected interval duration\ninterval_seconds={}\ndelta_secs={delta_secs}\nstdout:\n{stdout}\nstderr:\n{stderr}",
            s.interval_seconds
        );

        integrated_requests += s.requests_per_sec * s.interval_seconds;
        integrated_secs += s.interval_seconds;
        prev_elapsed = s.elapsed_seconds;
        prev_total = s.total_requests;
    }

    let integrated_abs_err = (integrated_requests - (wrkr_total_requests as f64)).abs();
    let integrated_rel_err = integrated_abs_err / (wrkr_total_requests as f64).max(1.0);

    anyhow::ensure!(
        integrated_rel_err <= 0.01 || integrated_abs_err <= 100.0,
        "progress totals mismatch\nintegrated_requests={integrated_requests}\nprogress_total_requests={wrkr_total_requests}\nabs_err={integrated_abs_err}\nrel_err={integrated_rel_err}\nprogress_samples={}\nstdout:\n{stdout}\nstderr:\n{stderr}",
        progress_samples.len()
    );

    // Expected average RPS for the progress window: total_requests / elapsed.
    // This should match `req_per_sec_avg` precisely (it is meant to be an accurate aggregate).
    let fact_rps = (wrkr_total_requests as f64) / integrated_secs.max(1e-9);

    // If throughput is extremely low for some reason, the relative tolerance becomes unstable.
    // In that case, require a small absolute error instead.
    let abs_err = (fact_rps - wrkr_rps_avg).abs();
    let rel_err = abs_err / fact_rps.max(1.0);

    anyhow::ensure!(
        rel_err <= 0.005 || abs_err <= 1.0,
        "rps mismatch\nwrkr_requests_per_sec={wrkr_rps_now}\nwrkr_req_per_sec_avg={wrkr_rps_avg}\nexpected_avg_rps={fact_rps}\nabs_err={abs_err}\nrel_err={rel_err}\nserver_seen={server_seen}\nelapsed_seconds={elapsed_seconds}\nprogress_elapsed_seconds={integrated_secs}\nwall_elapsed={wall:?}\nstdout:\n{stdout}\nstderr:\n{stderr}"
    );

    Ok(())
}
