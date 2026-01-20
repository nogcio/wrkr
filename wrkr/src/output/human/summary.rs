use std::path::Path;
use std::time::Duration;

use indicatif::HumanBytes;

use super::duration::format_duration_single;

pub(crate) fn render_run_header(
    script_path: &Path,
    scenarios: &[wrkr_core::runner::ScenarioConfig],
) {
    println!("execution: local");
    println!("   script: {}", script_path.display());

    let (scenario_count, max_vus, max_duration) = summarize_scenario_limits(scenarios);
    println!(
        "scenarios: (100.00%) {scenario_count} scenario(s), {max_vus} max VUs, {max_duration} max duration"
    );

    for s in scenarios {
        match &s.executor {
            wrkr_core::runner::ScenarioExecutor::ConstantVus { vus } => {
                let dur = s
                    .duration
                    .map(format_duration_single)
                    .unwrap_or_else(|| "-".to_string());
                println!("  * {}: {vus} looping VUs for {dur}", s.name);
            }
            wrkr_core::runner::ScenarioExecutor::RampingVus { start_vus, stages } => {
                let dur: Duration = stages.iter().map(|st| st.duration).sum();
                println!(
                    "  * {}: ramping VUs (start {start_vus}) for {}",
                    s.name,
                    format_duration_single(dur)
                );
            }
            wrkr_core::runner::ScenarioExecutor::RampingArrivalRate {
                start_rate,
                time_unit,
                pre_allocated_vus,
                max_vus,
                stages,
            } => {
                let dur: Duration = stages.iter().map(|st| st.duration).sum();
                println!(
                    "  * {}: ramping arrival rate (start {start_rate}/{}; pre-allocated VUs {pre_allocated_vus}; max VUs {max_vus}) for {}",
                    s.name,
                    format_duration_single(*time_unit),
                    format_duration_single(dur)
                );
            }
        }
    }

    println!();
}

#[derive(Debug, Clone, Copy)]
struct TrendStats {
    avg: Option<f64>,
    min: Option<f64>,
    med: Option<f64>,
    max: Option<f64>,
    p90: Option<f64>,
    p95: Option<f64>,
}

pub(crate) fn render_run_summary(summary: &wrkr_core::runner::RunSummary) {
    let elapsed_s = (summary.run_duration_ms as f64) / 1000.0;

    let http_reqs = metric_counter(summary, "http_reqs").unwrap_or(summary.requests_total as f64);
    let data_received =
        metric_counter(summary, "data_received").unwrap_or(summary.bytes_received_total as f64);
    let data_sent = metric_counter(summary, "data_sent").unwrap_or(summary.bytes_sent_total as f64);

    let iterations = metric_counter(summary, "iterations").unwrap_or(0.0);

    let dur = metric_trend_stats(summary, "http_req_duration");
    let iter_dur = metric_trend_stats(summary, "iteration_duration");

    let (_checks_rate, _checks_total, _checks_trues) = metric_rate_stats(summary, "checks");
    let (failed_rate, failed_total, failed_trues) = metric_rate_stats(summary, "http_req_failed");

    let http_failed = failed_trues;
    let http_ok = failed_total.saturating_sub(failed_trues);

    let rps = if elapsed_s > 0.0 {
        http_reqs / elapsed_s
    } else {
        0.0
    };

    let bps = if elapsed_s > 0.0 {
        data_received / elapsed_s
    } else {
        0.0
    };

    let bps_sent = if elapsed_s > 0.0 {
        data_sent / elapsed_s
    } else {
        0.0
    };

    println!("█ TOTAL RESULTS\n");

    render_checks_section(summary, elapsed_s);
    render_http_section(
        http_reqs,
        rps,
        failed_rate,
        failed_total,
        http_failed,
        http_ok,
        &dur,
    );
    render_execution_section(iterations, elapsed_s, &iter_dur);
    render_network_section(data_received, bps, data_sent, bps_sent);
    render_dropped_iterations(summary.dropped_iterations_total);
}

fn render_checks_section(summary: &wrkr_core::runner::RunSummary, elapsed_s: f64) {
    if summary.checks_total == 0 {
        return;
    }

    println!("  CHECKS");
    println!(
        "    checks_total....................: {} ({:.3}/s)",
        summary.checks_total,
        (summary.checks_total as f64) / elapsed_s.max(1e-9)
    );

    let ok = summary.checks_total.saturating_sub(summary.checks_failed);
    let ok_pct = if summary.checks_total == 0 {
        0.0
    } else {
        (ok as f64) / (summary.checks_total as f64) * 100.0
    };
    let bad_pct = 100.0 - ok_pct;

    println!(
        "    checks_succeeded................: {ok_pct:.2}% {ok} out of {}",
        summary.checks_total
    );
    println!(
        "    checks_failed...................: {bad_pct:.2}% {} out of {}\n",
        summary.checks_failed, summary.checks_total
    );

    for c in &summary.checks_by_name {
        if c.total == 0 {
            continue;
        }
        let symbol = if c.failed == 0 { "✓" } else { "✗" };
        println!("    {symbol} {}", c.name);
    }
    println!();
}

fn render_http_section(
    http_reqs: f64,
    rps: f64,
    failed_rate: Option<f64>,
    failed_total: u64,
    http_failed: u64,
    http_ok: u64,
    dur: &TrendStats,
) {
    println!("  HTTP");
    println!(
        "    http_req_duration..............: avg={avg} min={min} med={med} max={max} p(90)={p90} p(95)={p95}",
        avg = format_duration_ms_opt(dur.avg),
        min = format_duration_ms_opt(dur.min),
        med = format_duration_ms_opt(dur.med),
        max = format_duration_ms_opt(dur.max),
        p90 = format_duration_ms_opt(dur.p90),
        p95 = format_duration_ms_opt(dur.p95),
    );

    if http_ok + http_failed != 0 {
        let rate = failed_rate.unwrap_or_else(|| {
            if failed_total == 0 {
                0.0
            } else {
                (http_failed as f64) / (failed_total as f64)
            }
        });
        println!(
            "    http_req_failed.................: {pct:.2}%  {failed} out of {total}",
            pct = rate * 100.0,
            failed = http_failed,
            total = failed_total
        );
    }

    println!(
        "    http_reqs.......................: {total} ({rps:.5}/s)\n",
        total = http_reqs.round() as u64,
        rps = rps
    );
}

fn render_execution_section(iterations: f64, elapsed_s: f64, iter_dur: &TrendStats) {
    println!("  EXECUTION");
    if iterations != 0.0 {
        println!(
            "    iteration_duration..............: avg={avg} min={min} med={med} max={max} p(90)={p90} p(95)={p95}",
            avg = format_duration_ms_opt(iter_dur.avg),
            min = format_duration_ms_opt(iter_dur.min),
            med = format_duration_ms_opt(iter_dur.med),
            max = format_duration_ms_opt(iter_dur.max),
            p90 = format_duration_ms_opt(iter_dur.p90),
            p95 = format_duration_ms_opt(iter_dur.p95),
        );
        println!(
            "    iterations......................: {} ({:.5}/s)\n",
            iterations.round() as u64,
            (iterations / elapsed_s.max(1e-9))
        );
    } else {
        println!();
    }
}

fn render_network_section(data_received: f64, bps: f64, data_sent: f64, bps_sent: f64) {
    println!("  NETWORK");
    if data_received != 0.0 {
        println!(
            "    data_received...................: {}  {}/s",
            HumanBytes(data_received as u64),
            HumanBytes(bps.round() as u64)
        );
    }

    if data_sent != 0.0 {
        println!(
            "    data_sent.......................: {}  {}/s",
            HumanBytes(data_sent as u64),
            HumanBytes(bps_sent.round() as u64)
        );
    } else {
        println!();
    }
}

fn render_dropped_iterations(dropped_iterations_total: u64) {
    if dropped_iterations_total != 0 {
        println!("dropped_iterations: {dropped_iterations_total}");
    }
}

fn summarize_scenario_limits(
    scenarios: &[wrkr_core::runner::ScenarioConfig],
) -> (usize, u64, String) {
    let mut max_vus = 0u64;
    let mut max_dur = Duration::from_secs(0);

    for s in scenarios {
        let dur = s.duration.unwrap_or_else(|| match &s.executor {
            wrkr_core::runner::ScenarioExecutor::ConstantVus { .. } => Duration::from_secs(0),
            wrkr_core::runner::ScenarioExecutor::RampingVus { stages, .. } => {
                stages.iter().map(|st| st.duration).sum()
            }
            wrkr_core::runner::ScenarioExecutor::RampingArrivalRate { stages, .. } => {
                stages.iter().map(|st| st.duration).sum()
            }
        });
        max_dur = max_dur.max(dur);

        let vus = match &s.executor {
            wrkr_core::runner::ScenarioExecutor::ConstantVus { vus } => *vus,
            wrkr_core::runner::ScenarioExecutor::RampingVus { start_vus, stages } => {
                let stage_max = stages.iter().map(|st| st.target).max().unwrap_or(0);
                (*start_vus).max(stage_max)
            }
            wrkr_core::runner::ScenarioExecutor::RampingArrivalRate {
                max_vus,
                pre_allocated_vus,
                ..
            } => (*max_vus).max(*pre_allocated_vus),
        };
        max_vus = max_vus.max(vus);
    }

    (scenarios.len(), max_vus, format_duration_single(max_dur))
}

fn metric_counter(summary: &wrkr_core::runner::RunSummary, name: &str) -> Option<f64> {
    summary
        .metrics
        .iter()
        .find(|m| m.tags.is_empty() && m.name == name)
        .and_then(|m| match &m.values {
            wrkr_core::runner::MetricValues::Counter { value } => Some(*value),
            _ => None,
        })
}

fn metric_trend_stats(summary: &wrkr_core::runner::RunSummary, name: &str) -> TrendStats {
    let Some(m) = summary
        .metrics
        .iter()
        .find(|m| m.tags.is_empty() && m.name == name)
    else {
        return TrendStats {
            avg: None,
            min: None,
            med: None,
            max: None,
            p90: None,
            p95: None,
        };
    };

    match &m.values {
        wrkr_core::runner::MetricValues::Trend {
            min,
            max,
            avg,
            p50,
            p90,
            p95,
            p99: _,
            ..
        } => TrendStats {
            avg: *avg,
            min: *min,
            med: *p50,
            max: *max,
            p90: *p90,
            p95: *p95,
        },
        _ => TrendStats {
            avg: None,
            min: None,
            med: None,
            max: None,
            p90: None,
            p95: None,
        },
    }
}

fn metric_rate_stats(
    summary: &wrkr_core::runner::RunSummary,
    name: &str,
) -> (Option<f64>, u64, u64) {
    let Some(m) = summary
        .metrics
        .iter()
        .find(|m| m.tags.is_empty() && m.name == name)
    else {
        return (None, 0, 0);
    };

    match &m.values {
        wrkr_core::runner::MetricValues::Rate { rate, total, trues } => (*rate, *total, *trues),
        _ => (None, 0, 0),
    }
}

pub(crate) fn format_duration_ms_opt(ms: Option<f64>) -> String {
    let Some(ms) = ms else {
        return "-".to_string();
    };

    if !ms.is_finite() || ms < 0.0 {
        return "-".to_string();
    }

    let nanos = (ms * 1_000_000.0).round();
    let nanos = if nanos.is_finite() && nanos >= 0.0 {
        nanos as u64
    } else {
        0
    };

    let d = Duration::from_nanos(nanos);
    format_duration_single(d)
}
