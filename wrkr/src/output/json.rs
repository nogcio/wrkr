use serde::Serialize;
use std::collections::BTreeMap;
use std::collections::HashMap;
use std::io::Write as _;
use std::path::Path;
use std::sync::Arc;

use super::OutputFormatter;

pub(crate) struct JsonOutput;

impl OutputFormatter for JsonOutput {
    fn print_header(&self, _script_path: &Path, _scenarios: &[wrkr_core::ScenarioConfig]) {}

    fn progress(&self) -> Option<wrkr_core::ProgressFn> {
        Some(Arc::new(move |u| {
            let line = build_progress_line(&u);
            emit_json_line(&line);
        }))
    }

    fn print_summary(
        &self,
        summary: &wrkr_core::RunSummary,
        _metrics: Option<&[wrkr_core::MetricSeriesSummary]>,
    ) -> anyhow::Result<()> {
        let line = build_summary_line(summary);
        emit_json_line(&line);
        Ok(())
    }
}

#[derive(Debug, Serialize)]
pub(crate) struct JsonProgressLine {
    pub kind: &'static str,
    pub elapsed_secs: u64,
    pub interval_secs: f64,
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

fn build_progress_line(u: &wrkr_core::ProgressUpdate) -> JsonProgressLine {
    let (current_vus, _max_vus, _dropped_iterations_total) = scenario_progress_vus(&u.progress);

    JsonProgressLine {
        kind: "progress",
        elapsed_secs: u.elapsed.as_secs(),
        interval_secs: u.interval.as_secs_f64(),
        connections: current_vus,

        requests_per_sec: u.metrics.rps_now,
        bytes_received_per_sec: u.metrics.bytes_received_per_sec_now,
        bytes_sent_per_sec: u.metrics.bytes_sent_per_sec_now,

        total_requests: u.metrics.requests_total,
        total_bytes_received: u.metrics.bytes_received_total,
        total_bytes_sent: u.metrics.bytes_sent_total,
        checks_failed_total: u.metrics.checks_failed_total,

        latency_mean: u.metrics.latency_mean,
        latency_stdev: u.metrics.latency_stdev,
        latency_max: u.metrics.latency_max,
        latency_p50: u.metrics.latency_p50,
        latency_p75: u.metrics.latency_p75,
        latency_p90: u.metrics.latency_p90,
        latency_p99: u.metrics.latency_p99,
        latency_stdev_pct: u.metrics.latency_stdev_pct,

        checks_failed: u.metrics.checks_failed.clone(),

        req_per_sec_avg: u.metrics.req_per_sec_avg,
        req_per_sec_stdev: u.metrics.req_per_sec_stdev,
        req_per_sec_max: u.metrics.req_per_sec_max,
        req_per_sec_stdev_pct: u.metrics.req_per_sec_stdev_pct,
    }
}

#[derive(Debug, Serialize)]
pub(crate) struct JsonSummaryLine {
    pub kind: &'static str,
    pub scenarios: Vec<JsonScenarioSummary>,
    pub totals: JsonTotals,
}

#[derive(Debug, Serialize)]
pub(crate) struct JsonScenarioSummary {
    pub scenario: String,

    pub requests_total: u64,
    pub failed_requests_total: u64,
    pub bytes_received_total: u64,
    pub bytes_sent_total: u64,
    pub iterations_total: u64,

    pub checks_failed_total: u64,
    pub checks_failed: BTreeMap<String, u64>,

    pub latency: Option<JsonLatencySummary>,
}

#[derive(Debug, Serialize)]
pub(crate) struct JsonLatencySummary {
    pub p50: Option<f64>,
    pub p75: Option<f64>,
    pub p90: Option<f64>,
    pub p95: Option<f64>,
    pub p99: Option<f64>,
    pub min: Option<f64>,
    pub max: Option<f64>,
    pub mean: Option<f64>,
    pub stdev: Option<f64>,
    pub count: u64,
}

#[derive(Debug, Serialize, Default)]
pub(crate) struct JsonTotals {
    pub requests_total: u64,
    pub failed_requests_total: u64,
    pub bytes_received_total: u64,
    pub bytes_sent_total: u64,
    pub iterations_total: u64,
    pub checks_failed_total: u64,
}

fn build_summary_line(summary: &wrkr_core::RunSummary) -> JsonSummaryLine {
    let mut totals = JsonTotals::default();
    let scenarios = summary
        .scenarios
        .iter()
        .map(|s| {
            totals.requests_total = totals.requests_total.saturating_add(s.requests_total);
            totals.failed_requests_total = totals
                .failed_requests_total
                .saturating_add(s.failed_requests_total);
            totals.bytes_received_total = totals
                .bytes_received_total
                .saturating_add(s.bytes_received_total);
            totals.bytes_sent_total = totals.bytes_sent_total.saturating_add(s.bytes_sent_total);
            totals.iterations_total = totals.iterations_total.saturating_add(s.iterations_total);
            totals.checks_failed_total = totals
                .checks_failed_total
                .saturating_add(s.checks_failed_total);

            let checks_failed = s
                .checks_failed
                .iter()
                .map(|(k, v)| (k.clone(), *v))
                .collect::<BTreeMap<_, _>>();

            let latency = s.latency.as_ref().map(|l| JsonLatencySummary {
                p50: l.p50,
                p75: l.p75,
                p90: l.p90,
                p95: l.p95,
                p99: l.p99,
                min: l.min,
                max: l.max,
                mean: l.mean,
                stdev: l.stdev,
                count: l.count,
            });

            JsonScenarioSummary {
                scenario: s.scenario.clone(),
                requests_total: s.requests_total,
                failed_requests_total: s.failed_requests_total,
                bytes_received_total: s.bytes_received_total,
                bytes_sent_total: s.bytes_sent_total,
                iterations_total: s.iterations_total,
                checks_failed_total: s.checks_failed_total,
                checks_failed,
                latency,
            }
        })
        .collect::<Vec<_>>();

    JsonSummaryLine {
        kind: "summary",
        scenarios,
        totals,
    }
}

fn emit_json_line<T: Serialize>(line: &T) {
    let mut out = std::io::stdout().lock();
    if serde_json::to_writer(&mut out, line).is_ok() {
        let _ = writeln!(out);
    }
}

fn scenario_progress_vus(
    progress: &wrkr_core::ScenarioProgress,
) -> (u64, Option<u64>, Option<u64>) {
    match progress {
        wrkr_core::ScenarioProgress::ConstantVus { vus, .. } => (*vus, Some(*vus), None),
        wrkr_core::ScenarioProgress::RampingVus { stage, .. } => {
            let current = stage.as_ref().map(|s| s.current_target).unwrap_or(0);
            (current, None, None)
        }
        wrkr_core::ScenarioProgress::RampingArrivalRate {
            active_vus,
            max_vus,
            dropped_iterations_total,
            ..
        } => (*active_vus, Some(*max_vus), Some(*dropped_iterations_total)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::Value;

    #[test]
    fn progress_line_has_kind() {
        let mut checks_failed = HashMap::new();
        checks_failed.insert("x".to_string(), 1);

        let line = JsonProgressLine {
            kind: "progress",
            elapsed_secs: 1,
            interval_secs: 1.0,
            connections: 2,
            requests_per_sec: 3.0,
            bytes_received_per_sec: 4,
            bytes_sent_per_sec: 5,
            total_requests: 6,
            total_bytes_received: 7,
            total_bytes_sent: 8,
            checks_failed_total: 9,
            latency_mean: 10.0,
            latency_stdev: 11.0,
            latency_max: 12,
            latency_p50: 13,
            latency_p75: 14,
            latency_p90: 15,
            latency_p99: 16,
            latency_stdev_pct: 17.0,
            checks_failed,
            req_per_sec_avg: 18.0,
            req_per_sec_stdev: 19.0,
            req_per_sec_max: 20.0,
            req_per_sec_stdev_pct: 21.0,
        };

        let v: Value = match serde_json::to_value(&line) {
            Ok(v) => v,
            Err(err) => panic!("to_value failed: {err}"),
        };
        assert_eq!(v.get("kind").and_then(Value::as_str), Some("progress"));
    }

    #[test]
    fn summary_line_has_totals() {
        let summary = wrkr_core::RunSummary {
            scenarios: vec![wrkr_core::ScenarioSummary {
                scenario: "s1".to_string(),
                requests_total: 10,
                failed_requests_total: 2,
                bytes_received_total: 3,
                bytes_sent_total: 4,
                iterations_total: 5,
                checks_failed_total: 6,
                checks_failed: [("c1".to_string(), 6)].into_iter().collect(),
                latency: None,
            }],
        };

        let line = build_summary_line(&summary);
        let v: Value = match serde_json::to_value(&line) {
            Ok(v) => v,
            Err(err) => panic!("to_value failed: {err}"),
        };

        assert_eq!(v.get("kind").and_then(Value::as_str), Some("summary"));
        assert_eq!(
            v.pointer("/totals/requests_total").and_then(Value::as_u64),
            Some(10)
        );
        assert_eq!(
            v.pointer("/totals/checks_failed_total")
                .and_then(Value::as_u64),
            Some(6)
        );
        assert_eq!(
            v.pointer("/scenarios/0/scenario").and_then(Value::as_str),
            Some("s1")
        );
    }
}
