use std::collections::HashMap;

use wrkr_metrics::{KeyId, MetricId, Registry};

use crate::error::Result;
use crate::iteration_metrics::IterationMetricIds;
use crate::progress::LiveMetrics;
use crate::request_metrics::RequestMetricIds;
use crate::summary::{RunSummary, ScenarioSummary};

pub(crate) type RunningStats = wrkr_metrics::agg::RunningStats;

#[derive(Debug, Default, Clone, Copy)]
pub(crate) struct ScenarioSnapshot {
    pub(crate) requests_total: u64,
    pub(crate) bytes_received_total: u64,
    pub(crate) bytes_sent_total: u64,
    pub(crate) failed_requests_total: u64,
    pub(crate) checks_failed_total: u64,
    pub(crate) iterations_total: u64,
}

#[derive(Debug, Clone, Copy)]
struct TagKeys {
    scenario: KeyId,
    protocol: KeyId,
    status: KeyId,
    name: KeyId,
    fail: KeyId,
}

impl TagKeys {
    fn new(metrics: &Registry) -> Self {
        Self {
            scenario: metrics.resolve_key("scenario"),
            protocol: metrics.resolve_key("protocol"),
            status: metrics.resolve_key("status"),
            name: metrics.resolve_key("name"),
            fail: metrics.resolve_key("fail"),
        }
    }
}

fn compute_checks_failed(
    metrics: &Registry,
    checks_metric: MetricId,
    keys: TagKeys,
    scenario_value: KeyId,
) -> (u64, HashMap<String, u64>) {
    let grouped = metrics
        .query(checks_metric)
        .where_eq(keys.scenario, scenario_value)
        .where_eq(keys.status, keys.fail)
        .group_by([keys.name])
        .sum_counter();

    let mut total = 0u64;
    let mut by_name: HashMap<String, u64> = HashMap::new();

    for (tags, v) in grouped {
        total = total.saturating_add(v);

        let Some(name_id) = tags.get(keys.name) else {
            continue;
        };
        let Some(name) = metrics.resolve_key_id(name_id) else {
            continue;
        };

        by_name
            .entry(name.to_string())
            .and_modify(|cur| *cur = cur.saturating_add(v))
            .or_insert(v);
    }

    (total, by_name)
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct MetricComputer {
    request_ids: RequestMetricIds,
    iteration_ids: IterationMetricIds,
    checks_metric: MetricId,
    keys: TagKeys,
}

impl MetricComputer {
    pub(crate) fn new(
        metrics: &Registry,
        request_ids: RequestMetricIds,
        iteration_ids: IterationMetricIds,
        checks_metric: MetricId,
    ) -> Self {
        Self {
            request_ids,
            iteration_ids,
            checks_metric,
            keys: TagKeys::new(metrics),
        }
    }

    pub(crate) fn compute_live_metrics(
        &self,
        metrics: &Registry,
        scenario: &str,
        prev: Option<ScenarioSnapshot>,
        dt_secs: f64,
        rps_stats: &mut RunningStats,
    ) -> (LiveMetrics, ScenarioSnapshot) {
        let keys = self.keys;
        let scenario_value = metrics.resolve_key(scenario);

        let requests_total = metrics
            .query(self.request_ids.requests_total)
            .where_eq(keys.scenario, scenario_value)
            .sum_counter_total();

        let bytes_received_total = metrics
            .query(self.request_ids.bytes_received_total)
            .where_eq(keys.scenario, scenario_value)
            .sum_counter_total();

        let bytes_sent_total = metrics
            .query(self.request_ids.bytes_sent_total)
            .where_eq(keys.scenario, scenario_value)
            .sum_counter_total();

        let failed_requests_total = metrics
            .query(self.request_ids.errors_total)
            .where_eq(keys.scenario, scenario_value)
            .sum_counter_total();

        let iterations_total = metrics
            .query(self.iteration_ids.iterations_total)
            .where_eq(keys.scenario, scenario_value)
            .sum_counter_total();

        let (checks_failed_total, checks_failed) =
            compute_checks_failed(metrics, self.checks_metric, keys, scenario_value);

        // IMPORTANT: request_latency is recorded twice (overall + protocol-scoped).
        // For overall scenario latency we only want the series without `protocol`.
        let latency = metrics
            .query(self.request_ids.latency)
            .where_eq(keys.scenario, scenario_value)
            .where_missing(keys.protocol)
            .merge_histogram_summary_single();

        let snapshot = ScenarioSnapshot {
            requests_total,
            bytes_received_total,
            bytes_sent_total,
            failed_requests_total,
            checks_failed_total,
            iterations_total,
        };

        let dt = dt_secs.max(1e-9);
        let prev = prev.unwrap_or_default();

        let req_delta = snapshot.requests_total.saturating_sub(prev.requests_total);
        let bytes_in_delta = snapshot
            .bytes_received_total
            .saturating_sub(prev.bytes_received_total);
        let bytes_out_delta = snapshot
            .bytes_sent_total
            .saturating_sub(prev.bytes_sent_total);

        let rps_now = req_delta as f64 / dt;
        rps_stats.push(rps_now);

        let mut latency_mean = 0.0;
        let mut latency_stdev = 0.0;
        let mut latency_max = 0u64;
        let mut latency_p50 = 0u64;
        let mut latency_p75 = 0u64;
        let mut latency_p90 = 0u64;
        let mut latency_p99 = 0u64;
        let mut latency_stdev_pct = 0.0;

        if let Some(lat) = latency {
            latency_mean = lat.mean.unwrap_or(0.0);
            latency_stdev = lat.stdev.unwrap_or(0.0);
            latency_max = lat.max.unwrap_or(0.0) as u64;

            latency_p50 = lat.p50.unwrap_or(0.0) as u64;
            latency_p75 = lat.p75.unwrap_or(0.0) as u64;
            latency_p90 = lat.p90.unwrap_or(0.0) as u64;
            latency_p99 = lat.p99.unwrap_or(0.0) as u64;

            if latency_mean > 0.0 {
                latency_stdev_pct = (latency_stdev / latency_mean) * 100.0;
            }
        }

        let live = LiveMetrics {
            rps_now,
            bytes_received_per_sec_now: (bytes_in_delta as f64 / dt).round() as u64,
            bytes_sent_per_sec_now: (bytes_out_delta as f64 / dt).round() as u64,

            requests_total: snapshot.requests_total,
            bytes_received_total: snapshot.bytes_received_total,
            bytes_sent_total: snapshot.bytes_sent_total,
            failed_requests_total: snapshot.failed_requests_total,

            checks_failed_total: snapshot.checks_failed_total,
            checks_failed,

            req_per_sec_avg: rps_stats.mean(),
            req_per_sec_stdev: rps_stats.stdev(),
            req_per_sec_max: rps_stats.max(),
            req_per_sec_stdev_pct: rps_stats.stdev_pct(),

            latency_mean,
            latency_stdev,
            latency_max,
            latency_p50,
            latency_p75,
            latency_p90,
            latency_p99,
            latency_stdev_pct,

            iterations_total: snapshot.iterations_total,

            ..Default::default()
        };

        (live, snapshot)
    }

    pub(crate) fn compute_scenario_summary(
        &self,
        metrics: &Registry,
        scenario: &str,
    ) -> ScenarioSummary {
        let keys = self.keys;
        let scenario_value = metrics.resolve_key(scenario);

        let requests_total = metrics
            .query(self.request_ids.requests_total)
            .where_eq(keys.scenario, scenario_value)
            .sum_counter_total();

        let bytes_received_total = metrics
            .query(self.request_ids.bytes_received_total)
            .where_eq(keys.scenario, scenario_value)
            .sum_counter_total();

        let bytes_sent_total = metrics
            .query(self.request_ids.bytes_sent_total)
            .where_eq(keys.scenario, scenario_value)
            .sum_counter_total();

        let failed_requests_total = metrics
            .query(self.request_ids.errors_total)
            .where_eq(keys.scenario, scenario_value)
            .sum_counter_total();

        let iterations_total = metrics
            .query(self.iteration_ids.iterations_total)
            .where_eq(keys.scenario, scenario_value)
            .sum_counter_total();

        let (checks_failed_total, checks_failed) =
            compute_checks_failed(metrics, self.checks_metric, keys, scenario_value);

        let latency = metrics
            .query(self.request_ids.latency)
            .where_eq(keys.scenario, scenario_value)
            .where_missing(keys.protocol)
            .merge_histogram_summary_single();

        ScenarioSummary {
            scenario: scenario.to_string(),
            requests_total,
            failed_requests_total,
            bytes_received_total,
            bytes_sent_total,
            iterations_total,
            checks_failed_total,
            checks_failed,
            latency,
        }
    }
}

pub(crate) fn build_run_summary(
    metrics: &Registry,
    request_ids: RequestMetricIds,
    iteration_ids: IterationMetricIds,
    checks_metric: MetricId,
    scenario_names: &[String],
    thresholds: &[crate::ThresholdSet],
) -> Result<RunSummary> {
    let computer = MetricComputer::new(metrics, request_ids, iteration_ids, checks_metric);
    let scenarios = scenario_names
        .iter()
        .map(|name| computer.compute_scenario_summary(metrics, name))
        .collect();

    let metrics_summary = metrics.summarize();
    let threshold_violations = crate::thresholds_eval::evaluate_thresholds(metrics, thresholds)?;

    Ok(RunSummary {
        scenarios,
        metrics: metrics_summary,
        threshold_violations,
    })
}
