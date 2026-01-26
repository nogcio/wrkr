use serde::Serialize;
use std::collections::BTreeMap;
use std::io::Write as _;
use std::path::Path;
use std::sync::Arc;
use std::sync::OnceLock;

use super::OutputFormatter;

pub(crate) struct JsonOutput {
    scenarios: OnceLock<Vec<wrkr_core::ScenarioConfig>>,
}

const NDJSON_SCHEMA: &str = "wrkr.ndjson.v1";

impl JsonOutput {
    pub(crate) fn new() -> Self {
        Self {
            scenarios: OnceLock::new(),
        }
    }
}

impl OutputFormatter for JsonOutput {
    fn print_header(&self, _script_path: &Path, scenarios: &[wrkr_core::ScenarioConfig]) {
        let _ = self.scenarios.set(scenarios.to_vec());
    }

    fn progress(&self) -> Option<wrkr_core::ProgressFn> {
        Some(Arc::new(move |u| {
            let line = build_progress_line(&u);
            emit_json_line(&line);
        }))
    }

    fn print_summary(
        &self,
        summary: &wrkr_core::RunSummary,
        metrics: Option<&[wrkr_core::MetricSeriesSummary]>,
    ) -> anyhow::Result<()> {
        let line = build_summary_line(summary, metrics, self.scenarios.get().map(Vec::as_slice));
        emit_json_line(&line);
        Ok(())
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct JsonProgressLine {
    pub schema: &'static str,
    pub kind: &'static str,

    pub tick: u64,
    pub elapsed_seconds: f64,
    pub interval_seconds: f64,

    pub scenario: String,
    pub exec: String,

    pub executor: JsonProgressExecutor,
    pub metrics: JsonProgressMetrics,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct JsonProgressExecutor {
    pub kind: &'static str,
    pub vus_active: u64,
    pub vus_max: Option<u64>,
    pub dropped_iterations_total: Option<u64>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct JsonProgressMetrics {
    pub requests_per_sec: f64,
    pub bytes_received_per_sec: u64,
    pub bytes_sent_per_sec: u64,

    pub total_requests: u64,
    pub total_failed_requests: u64,
    pub total_iterations: u64,

    pub total_bytes_received: u64,
    pub total_bytes_sent: u64,

    pub checks_failed_total: u64,

    pub latency_seconds: JsonProgressLatencySeconds,

    pub req_per_sec_avg: f64,
    pub req_per_sec_stdev: f64,
    pub req_per_sec_max: f64,
    pub req_per_sec_stdev_pct: f64,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct JsonProgressLatencySeconds {
    pub mean: f64,
    pub stdev: f64,
    pub max: f64,
    pub p50: f64,
    pub p75: f64,
    pub p90: f64,
    pub p99: f64,
    pub stdev_pct: f64,
}

fn build_progress_line(u: &wrkr_core::ProgressUpdate) -> JsonProgressLine {
    let (current_vus, max_vus, dropped_iterations_total) = scenario_progress_vus(&u.progress);

    let executor_kind = scenario_progress_kind(&u.progress);

    let us_to_secs = |v_us: f64| v_us / 1_000_000.0;
    let u64_us_to_secs = |v_us: u64| (v_us as f64) / 1_000_000.0;

    JsonProgressLine {
        schema: NDJSON_SCHEMA,
        kind: "progress",

        tick: u.tick,
        elapsed_seconds: u.elapsed.as_secs_f64(),
        interval_seconds: u.interval.as_secs_f64(),

        scenario: u.scenario.clone(),
        exec: u.exec.clone(),

        executor: JsonProgressExecutor {
            kind: executor_kind,
            vus_active: current_vus,
            vus_max: max_vus,
            dropped_iterations_total,
        },
        metrics: JsonProgressMetrics {
            requests_per_sec: u.metrics.rps_now,
            bytes_received_per_sec: u.metrics.bytes_received_per_sec_now,
            bytes_sent_per_sec: u.metrics.bytes_sent_per_sec_now,

            total_requests: u.metrics.requests_total,
            total_failed_requests: u.metrics.failed_requests_total,
            total_iterations: u.metrics.iterations_total,

            total_bytes_received: u.metrics.bytes_received_total,
            total_bytes_sent: u.metrics.bytes_sent_total,

            checks_failed_total: u.metrics.checks_failed_total,

            latency_seconds: JsonProgressLatencySeconds {
                mean: us_to_secs(u.metrics.latency_mean),
                stdev: us_to_secs(u.metrics.latency_stdev),
                max: u64_us_to_secs(u.metrics.latency_max),
                p50: u64_us_to_secs(u.metrics.latency_p50),
                p75: u64_us_to_secs(u.metrics.latency_p75),
                p90: u64_us_to_secs(u.metrics.latency_p90),
                p99: u64_us_to_secs(u.metrics.latency_p99),
                stdev_pct: u.metrics.latency_stdev_pct,
            },

            req_per_sec_avg: u.metrics.req_per_sec_avg,
            req_per_sec_stdev: u.metrics.req_per_sec_stdev,
            req_per_sec_max: u.metrics.req_per_sec_max,
            req_per_sec_stdev_pct: u.metrics.req_per_sec_stdev_pct,
        },
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct JsonSummaryLine {
    pub schema: &'static str,
    pub kind: &'static str,
    pub scenarios: Vec<JsonScenarioSummary>,
    pub totals: JsonTotals,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct JsonScenarioSummary {
    pub scenario: String,

    pub exec: Option<String>,
    pub executor: Option<JsonScenarioExecutorConfig>,

    pub requests_total: u64,
    pub failed_requests_total: u64,
    pub bytes_received_total: u64,
    pub bytes_sent_total: u64,
    pub iterations_total: u64,

    pub checks: Option<JsonChecksSummary>,

    pub latency_seconds: Option<JsonLatencySummarySeconds>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct JsonScenarioExecutorConfig {
    pub kind: &'static str,

    pub vus: Option<u64>,

    pub start_vus: Option<u64>,
    pub stages: Option<Vec<JsonStage>>, // ramping-vus / ramping-arrival-rate

    pub start_rate: Option<u64>,
    pub time_unit_seconds: Option<f64>,
    pub pre_allocated_vus: Option<u64>,
    pub max_vus: Option<u64>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct JsonStage {
    pub duration_seconds: f64,
    pub target: u64,
}

#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub(crate) struct JsonChecksSummary {
    pub total: u64,
    pub passed: u64,
    pub failed: u64,
    pub by_series: Vec<JsonCheckSeriesSummary>,
}

#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub(crate) struct JsonCheckSeriesSummary {
    pub name: String,
    pub group: Option<String>,
    pub tags: BTreeMap<String, String>,
    pub passed: u64,
    pub failed: u64,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct JsonLatencySummarySeconds {
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
#[serde(rename_all = "camelCase")]
pub(crate) struct JsonTotals {
    pub requests_total: u64,
    pub failed_requests_total: u64,
    pub bytes_received_total: u64,
    pub bytes_sent_total: u64,
    pub iterations_total: u64,
    pub checks_failed_total: u64,
}

fn build_summary_line(
    summary: &wrkr_core::RunSummary,
    metrics: Option<&[wrkr_core::MetricSeriesSummary]>,
    scenarios: Option<&[wrkr_core::ScenarioConfig]>,
) -> JsonSummaryLine {
    let mut totals = JsonTotals::default();

    let checks_by_scenario = metrics
        .map(parse_checks_from_metric_series)
        .unwrap_or_default();

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

            let checks = checks_by_scenario.get(s.scenario.as_str()).cloned();
            totals.checks_failed_total = totals.checks_failed_total.saturating_add(
                checks
                    .as_ref()
                    .map(|c| c.failed)
                    .unwrap_or(s.checks_failed_total),
            );

            let (exec, executor) = scenarios
                .and_then(|cfgs| cfgs.iter().find(|c| c.metrics_ctx.scenario() == s.scenario))
                .map(|cfg| (Some(cfg.exec.clone()), Some(executor_config(cfg))))
                .unwrap_or((None, None));

            let us_to_secs_opt = |v: Option<f64>| v.map(|x| x / 1_000_000.0);

            let latency_seconds = s.latency.as_ref().map(|l| JsonLatencySummarySeconds {
                p50: us_to_secs_opt(l.p50),
                p75: us_to_secs_opt(l.p75),
                p90: us_to_secs_opt(l.p90),
                p95: us_to_secs_opt(l.p95),
                p99: us_to_secs_opt(l.p99),
                min: us_to_secs_opt(l.min),
                max: us_to_secs_opt(l.max),
                mean: us_to_secs_opt(l.mean),
                stdev: us_to_secs_opt(l.stdev),
                count: l.count,
            });

            JsonScenarioSummary {
                scenario: s.scenario.clone(),

                exec,
                executor,

                requests_total: s.requests_total,
                failed_requests_total: s.failed_requests_total,
                bytes_received_total: s.bytes_received_total,
                bytes_sent_total: s.bytes_sent_total,
                iterations_total: s.iterations_total,

                checks,
                latency_seconds,
            }
        })
        .collect::<Vec<_>>();

    JsonSummaryLine {
        schema: NDJSON_SCHEMA,
        kind: "summary",
        scenarios,
        totals,
    }
}

fn executor_config(cfg: &wrkr_core::ScenarioConfig) -> JsonScenarioExecutorConfig {
    match &cfg.executor {
        wrkr_core::ScenarioExecutor::ConstantVus { vus } => JsonScenarioExecutorConfig {
            kind: "constant-vus",
            vus: Some(*vus),
            start_vus: None,
            stages: None,
            start_rate: None,
            time_unit_seconds: None,
            pre_allocated_vus: None,
            max_vus: None,
        },
        wrkr_core::ScenarioExecutor::RampingVus { start_vus, stages } => {
            JsonScenarioExecutorConfig {
                kind: "ramping-vus",
                vus: None,
                start_vus: Some(*start_vus),
                stages: Some(
                    stages
                        .iter()
                        .map(|s| JsonStage {
                            duration_seconds: s.duration.as_secs_f64(),
                            target: s.target,
                        })
                        .collect(),
                ),
                start_rate: None,
                time_unit_seconds: None,
                pre_allocated_vus: None,
                max_vus: None,
            }
        }
        wrkr_core::ScenarioExecutor::RampingArrivalRate {
            start_rate,
            time_unit,
            pre_allocated_vus,
            max_vus,
            stages,
        } => JsonScenarioExecutorConfig {
            kind: "ramping-arrival-rate",
            vus: None,
            start_vus: None,
            stages: Some(
                stages
                    .iter()
                    .map(|s| JsonStage {
                        duration_seconds: s.duration.as_secs_f64(),
                        target: s.target,
                    })
                    .collect(),
            ),
            start_rate: Some(*start_rate),
            time_unit_seconds: Some(time_unit.as_secs_f64()),
            pre_allocated_vus: Some(*pre_allocated_vus),
            max_vus: Some(*max_vus),
        },
    }
}

#[derive(Debug, Default)]
struct ChecksByScenario {
    by_scenario: std::collections::BTreeMap<String, JsonChecksSummary>,
}

impl ChecksByScenario {
    fn get(&self, scenario: &str) -> Option<&JsonChecksSummary> {
        self.by_scenario.get(scenario)
    }
}

fn parse_checks_from_metric_series(metrics: &[wrkr_core::MetricSeriesSummary]) -> ChecksByScenario {
    use wrkr_core::{MetricKind, MetricValue};

    #[derive(Debug, Clone, Hash, PartialEq, Eq)]
    struct SeriesKey {
        scenario: String,
        name: String,
        group: Option<String>,
        tags: Vec<(String, String)>,
    }

    #[derive(Debug, Default, Clone, Copy)]
    struct Acc {
        passed: u64,
        failed: u64,
    }

    let mut by_key: std::collections::HashMap<SeriesKey, Acc> = std::collections::HashMap::new();

    for m in metrics {
        if m.name != "checks" {
            continue;
        }
        if m.kind != MetricKind::Counter {
            continue;
        }
        let MetricValue::Counter(n) = &m.values else {
            continue;
        };

        let mut scenario: Option<&str> = None;
        let mut name: Option<&str> = None;
        let mut status: Option<&str> = None;
        let mut group: Option<&str> = None;
        let mut tags: Vec<(String, String)> = Vec::new();

        for (k, v) in &m.tags {
            match k.as_str() {
                "scenario" => scenario = Some(v.as_str()),
                "name" => name = Some(v.as_str()),
                "status" => status = Some(v.as_str()),
                "group" => group = Some(v.as_str()),
                _ => tags.push((k.clone(), v.clone())),
            }
        }

        let Some(scenario) = scenario else {
            continue;
        };
        let Some(name) = name else {
            continue;
        };
        let Some(status) = status else {
            continue;
        };

        tags.sort();

        let key = SeriesKey {
            scenario: scenario.to_string(),
            name: name.to_string(),
            group: group.map(str::to_string),
            tags,
        };

        let acc = by_key.entry(key).or_default();
        match status {
            "pass" => acc.passed = acc.passed.saturating_add(*n),
            "fail" => acc.failed = acc.failed.saturating_add(*n),
            _ => {}
        }
    }

    let mut by_scenario: std::collections::BTreeMap<String, JsonChecksSummary> =
        std::collections::BTreeMap::new();

    for (k, v) in by_key {
        let tags_map = k.tags.into_iter().collect::<BTreeMap<String, String>>();

        let series = JsonCheckSeriesSummary {
            name: k.name,
            group: k.group,
            tags: tags_map,
            passed: v.passed,
            failed: v.failed,
        };

        let entry = by_scenario
            .entry(k.scenario)
            .or_insert_with(|| JsonChecksSummary {
                total: 0,
                passed: 0,
                failed: 0,
                by_series: Vec::new(),
            });

        entry.passed = entry.passed.saturating_add(series.passed);
        entry.failed = entry.failed.saturating_add(series.failed);
        entry.total = entry
            .total
            .saturating_add(series.passed.saturating_add(series.failed));
        entry.by_series.push(series);
    }

    for v in by_scenario.values_mut() {
        v.by_series.sort_by(|a, b| {
            let ak = (
                a.name.as_str(),
                a.group.as_deref().unwrap_or(""),
                a.tags
                    .iter()
                    .map(|(k, v)| format!("{k}={v}"))
                    .collect::<Vec<_>>()
                    .join(","),
            );
            let bk = (
                b.name.as_str(),
                b.group.as_deref().unwrap_or(""),
                b.tags
                    .iter()
                    .map(|(k, v)| format!("{k}={v}"))
                    .collect::<Vec<_>>()
                    .join(","),
            );
            ak.cmp(&bk)
        });
    }

    ChecksByScenario { by_scenario }
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

fn scenario_progress_kind(progress: &wrkr_core::ScenarioProgress) -> &'static str {
    match progress {
        wrkr_core::ScenarioProgress::ConstantVus { .. } => "constant-vus",
        wrkr_core::ScenarioProgress::RampingVus { .. } => "ramping-vus",
        wrkr_core::ScenarioProgress::RampingArrivalRate { .. } => "ramping-arrival-rate",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::Value;

    #[test]
    fn progress_line_has_kind() {
        let line = JsonProgressLine {
            schema: NDJSON_SCHEMA,
            kind: "progress",
            tick: 1,
            elapsed_seconds: 1.0,
            interval_seconds: 1.0,
            scenario: "s1".to_string(),
            exec: "Default".to_string(),
            executor: JsonProgressExecutor {
                kind: "constant-vus",
                vus_active: 2,
                vus_max: Some(2),
                dropped_iterations_total: None,
            },
            metrics: JsonProgressMetrics {
                requests_per_sec: 3.0,
                bytes_received_per_sec: 4,
                bytes_sent_per_sec: 5,
                total_requests: 6,
                total_failed_requests: 0,
                total_iterations: 6,
                total_bytes_received: 7,
                total_bytes_sent: 8,
                checks_failed_total: 9,
                latency_seconds: JsonProgressLatencySeconds {
                    mean: 0.01,
                    stdev: 0.02,
                    max: 0.03,
                    p50: 0.04,
                    p75: 0.05,
                    p90: 0.06,
                    p99: 0.07,
                    stdev_pct: 7.0,
                },
                req_per_sec_avg: 18.0,
                req_per_sec_stdev: 19.0,
                req_per_sec_max: 20.0,
                req_per_sec_stdev_pct: 21.0,
            },
        };

        let v: Value = match serde_json::to_value(&line) {
            Ok(v) => v,
            Err(err) => panic!("to_value failed: {err}"),
        };
        assert_eq!(v.get("schema").and_then(Value::as_str), Some(NDJSON_SCHEMA));
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

        let line = build_summary_line(&summary, None, None);
        let v: Value = match serde_json::to_value(&line) {
            Ok(v) => v,
            Err(err) => panic!("to_value failed: {err}"),
        };

        assert_eq!(v.get("schema").and_then(Value::as_str), Some(NDJSON_SCHEMA));
        assert_eq!(v.get("kind").and_then(Value::as_str), Some("summary"));
        assert_eq!(
            v.pointer("/totals/requestsTotal").and_then(Value::as_u64),
            Some(10)
        );
        assert_eq!(
            v.pointer("/totals/checksFailedTotal")
                .and_then(Value::as_u64),
            Some(6)
        );
        assert_eq!(
            v.pointer("/scenarios/0/scenario").and_then(Value::as_str),
            Some("s1")
        );
    }
}
