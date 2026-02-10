use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use tokio::sync::broadcast;
use wrkr_metrics::{KeyId, MetricId, Registry};

use super::types::{DashboardEvent, DashboardPoint, DashboardSnapshot};

const DASHBOARD_SCHEMA: &str = "wrkr.dashboard.v1";

#[derive(Debug, Clone)]
pub(crate) struct DashboardCollector {
    source: MetricsSource,
    inner: Arc<Mutex<Inner>>,
    tx: broadcast::Sender<DashboardEvent>,
}

#[derive(Debug, Clone)]
struct MetricsSource {
    metrics: Arc<Registry>,
    request_metrics: wrkr_core::RequestMetricIds,
    iteration_metrics: wrkr_core::IterationMetricIds,
    checks_metric: MetricId,
    keys: TagKeys,
}

#[derive(Debug, Clone, Copy)]
struct TagKeys {
    protocol: KeyId,
    status: KeyId,
    fail: KeyId,
}

impl TagKeys {
    fn new(metrics: &Registry) -> Self {
        Self {
            protocol: metrics.resolve_key("protocol"),
            status: metrics.resolve_key("status"),
            fail: metrics.resolve_key("fail"),
        }
    }
}

#[derive(Debug)]
struct Inner {
    last_tick_sampled: u64,

    scenario_vus: HashMap<String, ScenarioVus>,

    prev_iters_total: u64,
    prev_reqs_total: u64,
    prev_failed_reqs_total: u64,
    prev_bytes_received_total: u64,
    prev_bytes_sent_total: u64,

    vus_max_seen: u64,

    // Full 1Hz series ("full-resolution" per issue; current core tick is 1Hz).
    points: Vec<DashboardPoint>,
}

#[derive(Debug, Clone, Copy, Default)]
struct ScenarioVus {
    current: u64,
    max_seen: u64,
}

impl DashboardCollector {
    pub(crate) fn new(run_ctx: &wrkr_core::RunScenariosContext) -> Self {
        let (tx, _) = broadcast::channel(64);
        let keys = TagKeys::new(run_ctx.metrics.as_ref());
        Self {
            source: MetricsSource {
                metrics: run_ctx.metrics.clone(),
                request_metrics: run_ctx.request_metrics,
                iteration_metrics: run_ctx.iteration_metrics,
                checks_metric: run_ctx.checks_metric,
                keys,
            },
            inner: Arc::new(Mutex::new(Inner {
                last_tick_sampled: 0,
                scenario_vus: HashMap::new(),
                prev_iters_total: 0,
                prev_reqs_total: 0,
                prev_failed_reqs_total: 0,
                prev_bytes_received_total: 0,
                prev_bytes_sent_total: 0,
                vus_max_seen: 0,
                points: Vec::new(),
            })),
            tx,
        }
    }

    pub(crate) fn subscribe(&self) -> broadcast::Receiver<DashboardEvent> {
        self.tx.subscribe()
    }

    pub(crate) fn snapshot(&self) -> DashboardSnapshot {
        let inner = self
            .inner
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        DashboardSnapshot {
            schema: DASHBOARD_SCHEMA,
            points: inner.points.clone(),
        }
    }

    pub(crate) fn progress_fn(&self) -> wrkr_core::ProgressFn {
        let this = self.clone();
        Arc::new(move |u| {
            this.record(&u);
        })
    }

    pub(crate) fn record(&self, u: &wrkr_core::ProgressUpdate) {
        let mut inner = self
            .inner
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());

        let (vus, vus_max_candidate) = match &u.progress {
            wrkr_core::ScenarioProgress::ConstantVus { vus, .. } => (*vus, *vus),
            wrkr_core::ScenarioProgress::RampingVus { stage, .. } => {
                let current = stage.as_ref().map(|s| s.current_target).unwrap_or_default();
                (current, current)
            }
            wrkr_core::ScenarioProgress::RampingArrivalRate {
                active_vus,
                max_vus,
                ..
            } => (*active_vus, *max_vus),
        };

        let scenario_entry = inner.scenario_vus.entry(u.scenario.clone()).or_default();
        scenario_entry.current = vus;
        scenario_entry.max_seen = scenario_entry.max_seen.max(vus_max_candidate).max(vus);

        // We may receive multiple per-scenario updates for the same tick; sample once.
        if u.tick == inner.last_tick_sampled {
            return;
        }
        inner.last_tick_sampled = u.tick;

        let overall_vus: u64 = inner.scenario_vus.values().map(|v| v.current).sum();
        let overall_vus_max: u64 = inner.scenario_vus.values().map(|v| v.max_seen).sum();
        inner.vus_max_seen = inner.vus_max_seen.max(overall_vus_max).max(overall_vus);
        let vus_max = inner.vus_max_seen;

        let metrics = &self.source.metrics;
        let request_ids = self.source.request_metrics;
        let iteration_ids = self.source.iteration_metrics;
        let checks_metric = self.source.checks_metric;
        let keys = self.source.keys;

        let requests_total = metrics
            .query(request_ids.requests_total)
            .sum_counter_total();
        let failed_requests_total = metrics.query(request_ids.errors_total).sum_counter_total();
        let iterations_total = metrics
            .query(iteration_ids.iterations_total)
            .sum_counter_total();
        let bytes_received_total = metrics
            .query(request_ids.bytes_received_total)
            .sum_counter_total();
        let bytes_sent_total = metrics
            .query(request_ids.bytes_sent_total)
            .sum_counter_total();

        let checks_failed_total = metrics
            .query(checks_metric)
            .where_eq(keys.status, keys.fail)
            .sum_counter_total();

        let latency = metrics
            .query(request_ids.latency)
            .where_missing(keys.protocol)
            .merge_histogram_summary_single();
        let iteration_duration = metrics
            .query(iteration_ids.iteration_duration)
            .merge_histogram_summary_single();

        let prev_iters = inner.prev_iters_total;
        let prev_reqs = inner.prev_reqs_total;
        let prev_failed = inner.prev_failed_reqs_total;
        let prev_bytes_in = inner.prev_bytes_received_total;
        let prev_bytes_out = inner.prev_bytes_sent_total;

        let iter_delta = iterations_total.saturating_sub(prev_iters);
        let req_delta = requests_total.saturating_sub(prev_reqs);
        let failed_delta = failed_requests_total.saturating_sub(prev_failed);
        let bytes_in_delta = bytes_received_total.saturating_sub(prev_bytes_in);
        let bytes_out_delta = bytes_sent_total.saturating_sub(prev_bytes_out);

        inner.prev_iters_total = iterations_total;
        inner.prev_reqs_total = requests_total;
        inner.prev_failed_reqs_total = failed_requests_total;
        inner.prev_bytes_received_total = bytes_received_total;
        inner.prev_bytes_sent_total = bytes_sent_total;
        let dt = u.interval.as_secs_f64().max(1e-9);

        let error_rate = if req_delta > 0 {
            (failed_delta as f64) / (req_delta as f64)
        } else {
            0.0
        };

        let (
            http_req_duration_avg_ms,
            http_req_duration_p90_ms,
            http_req_duration_p95_ms,
            http_req_duration_p99_ms,
        ) = if let Some(lat) = latency {
            let avg = (lat.count > 0).then(|| lat.mean.unwrap_or(0.0) / 1000.0);
            let p90 = lat.p90.map(|v| v / 1000.0);
            let p95 = lat.p95.map(|v| v / 1000.0);
            let p99 = lat.p99.map(|v| v / 1000.0);
            (avg, p90, p95, p99)
        } else {
            (None, None, None, None)
        };

        let (
            iteration_duration_avg_ms,
            iteration_duration_p90_ms,
            iteration_duration_p95_ms,
            iteration_duration_p99_ms,
        ) = if let Some(iter_dur) = iteration_duration {
            let avg = (iter_dur.count > 0).then(|| iter_dur.mean.unwrap_or(0.0) / 1000.0);
            let p90 = iter_dur.p90.map(|v| v / 1000.0);
            let p95 = iter_dur.p95.map(|v| v / 1000.0);
            let p99 = iter_dur.p99.map(|v| v / 1000.0);
            (avg, p90, p95, p99)
        } else {
            (None, None, None, None)
        };

        let point = DashboardPoint {
            tick: u.tick,
            elapsed_seconds: u.elapsed.as_secs_f64(),
            interval_seconds: u.interval.as_secs_f64(),

            vus: overall_vus,
            vus_max,
            requests_per_sec: (req_delta as f64) / dt,
            iterations_per_sec: (iter_delta as f64) / dt,
            bytes_received_per_sec: ((bytes_in_delta as f64) / dt).round() as u64,
            bytes_sent_per_sec: ((bytes_out_delta as f64) / dt).round() as u64,
            requests_total,
            failed_requests_total,
            iterations_total,
            bytes_received_total,
            bytes_sent_total,
            checks_failed_total,
            error_rate,

            http_req_duration_avg_ms,
            http_req_duration_p90_ms,
            http_req_duration_p95_ms,
            http_req_duration_p99_ms,

            iteration_duration_avg_ms,
            iteration_duration_p90_ms,
            iteration_duration_p95_ms,
            iteration_duration_p99_ms,
        };

        inner.points.push(point.clone());

        let _ = self.tx.send(DashboardEvent::Tick {
            point: Box::new(point),
        });
    }

    pub(crate) fn mark_done(&self) {
        let _ = self.tx.send(DashboardEvent::Done);
    }

    pub(crate) fn snapshot_event(&self) -> DashboardEvent {
        DashboardEvent::Snapshot {
            snapshot: self.snapshot(),
        }
    }
}
