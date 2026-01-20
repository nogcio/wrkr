use serde::Serialize;
use std::collections::HashMap;
use std::io::Write as _;
use std::path::Path;
use std::sync::Arc;

use super::OutputFormatter;

pub(crate) struct JsonOutput;

impl OutputFormatter for JsonOutput {
    fn print_header(&self, _script_path: &Path, _scenarios: &[wrkr_core::runner::ScenarioConfig]) {}

    fn progress(&self) -> Option<wrkr_core::runner::ProgressFn> {
        Some(Arc::new(move |u| {
            let line = build_progress_line(&u);
            emit_json_line(&line);
        }))
    }

    fn print_summary(&self, _summary: &wrkr_core::runner::RunSummary) -> anyhow::Result<()> {
        Ok(())
    }
}

#[derive(Debug, Serialize)]
pub(crate) struct JsonProgressLine {
    pub elapsed_secs: u64,
    pub connections: u64,

    pub requests_per_sec: f64,
    pub bytes_received_per_sec: u64,
    pub bytes_sent_per_sec: u64,

    pub total_requests: u64,
    pub total_bytes_received: u64,
    pub total_bytes_sent: u64,
    pub checks_failed_total: u64,

    pub latency_mean: f64,
    pub latency_stdev: f64,
    pub latency_max: u64,
    pub latency_p50: u64,
    pub latency_p75: u64,
    pub latency_p90: u64,
    pub latency_p99: u64,
    pub latency_stdev_pct: f64,

    pub checks_failed: HashMap<String, u64>,

    pub req_per_sec_avg: f64,
    pub req_per_sec_stdev: f64,
    pub req_per_sec_max: f64,
    pub req_per_sec_stdev_pct: f64,
}

fn build_progress_line(u: &wrkr_core::runner::ProgressUpdate) -> JsonProgressLine {
    let (current_vus, _max_vus, _dropped_iterations_total) = scenario_progress_vus(&u.progress);

    JsonProgressLine {
        elapsed_secs: u.elapsed.as_secs(),
        connections: current_vus,

        requests_per_sec: u.metrics.rps_now,
        bytes_received_per_sec: u.metrics.bytes_received_per_sec_now,
        bytes_sent_per_sec: u.metrics.bytes_sent_per_sec_now,

        total_requests: u.metrics.requests_total,
        total_bytes_received: u.metrics.bytes_received_total,
        total_bytes_sent: u.metrics.bytes_sent_total,
        checks_failed_total: u.metrics.checks_failed_total,

        latency_mean: u.metrics.latency_mean_ms,
        latency_stdev: u.metrics.latency_stdev_ms,
        latency_max: u.metrics.latency_max_ms,
        latency_p50: u.metrics.latency_p50_ms,
        latency_p75: u.metrics.latency_p75_ms,
        latency_p90: u.metrics.latency_p90_ms,
        latency_p99: u.metrics.latency_p99_ms,
        latency_stdev_pct: u.metrics.latency_stdev_pct,

        checks_failed: u.metrics.checks_failed.clone(),

        req_per_sec_avg: u.metrics.req_per_sec_avg,
        req_per_sec_stdev: u.metrics.req_per_sec_stdev,
        req_per_sec_max: u.metrics.req_per_sec_max,
        req_per_sec_stdev_pct: u.metrics.req_per_sec_stdev_pct,
    }
}

fn emit_json_line(line: &JsonProgressLine) {
    let mut out = std::io::stdout().lock();
    if serde_json::to_writer(&mut out, line).is_ok() {
        let _ = writeln!(out);
    }
}

fn scenario_progress_vus(
    progress: &wrkr_core::runner::ScenarioProgress,
) -> (u64, Option<u64>, Option<u64>) {
    match progress {
        wrkr_core::runner::ScenarioProgress::ConstantVus { vus, .. } => (*vus, Some(*vus), None),
        wrkr_core::runner::ScenarioProgress::RampingVus { stage, .. } => {
            let current = stage.as_ref().map(|s| s.current_target).unwrap_or(0);
            (current, None, None)
        }
        wrkr_core::runner::ScenarioProgress::RampingArrivalRate {
            active_vus,
            max_vus,
            dropped_iterations_total,
            ..
        } => (*active_vus, Some(*max_vus), Some(*dropped_iterations_total)),
    }
}
