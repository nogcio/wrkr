use hdrhistogram::Histogram;
use std::collections::HashMap;
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

use super::metrics::{MetricHandle, MetricKind, MetricSeriesSummary, MetricsRegistry};

#[derive(Debug, Default)]
struct CheckCounters {
    total: AtomicU64,
    failed: AtomicU64,
}

#[derive(Debug, Clone)]
pub struct CheckHandle {
    counters: Arc<CheckCounters>,
}

#[derive(Debug, Clone)]
pub struct CheckSummary {
    pub name: String,
    pub total: u64,
    pub failed: u64,
}

#[derive(Debug, Clone, Copy)]
pub struct HttpRequestMeta<'a> {
    pub method: &'a str,
    pub name: &'a str,
    pub status: Option<u16>,
    /// If set, the request failed due to a transport error.
    pub transport_error_kind: Option<crate::HttpTransportErrorKind>,
    pub elapsed: Duration,
    pub bytes_received: u64,
    pub bytes_sent: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, strum::Display, strum::EnumString)]
#[strum(serialize_all = "SCREAMING_SNAKE_CASE")]
pub enum GrpcCallKind {
    Unary,
    ServerStreaming,
    ClientStreaming,
    BidiStreaming,
}

#[derive(Debug, Clone, Copy)]
pub struct GrpcRequestMeta<'a> {
    /// gRPC call kind. (v1: only unary)
    pub method: GrpcCallKind,
    pub name: &'a str,
    /// gRPC status code (0..=16). `None` means a transport failure before status.
    pub status: Option<u16>,
    /// If set, the request failed due to a transport error.
    pub transport_error_kind: Option<crate::GrpcTransportErrorKind>,
    pub elapsed: Duration,
    pub bytes_received: u64,
    pub bytes_sent: u64,
}

#[derive(Debug, Clone)]
pub struct RunSummary {
    pub requests_total: u64,
    pub dropped_iterations_total: u64,
    pub checks_total: u64,
    pub checks_failed: u64,
    pub checks_by_name: Vec<CheckSummary>,
    pub bytes_received_total: u64,
    pub bytes_sent_total: u64,
    pub run_duration_ms: u64,
    pub rps: f64,
    pub req_per_sec_avg: f64,
    pub req_per_sec_stdev: f64,
    pub req_per_sec_max: f64,
    pub req_per_sec_stdev_pct: f64,
    pub latency_p50_ms: Option<f64>,
    pub latency_p95_ms: Option<f64>,
    pub latency_p75_ms: Option<f64>,
    pub latency_p90_ms: Option<f64>,
    pub latency_p99_ms: Option<f64>,
    pub latency_mean_ms: Option<f64>,
    pub latency_stdev_ms: Option<f64>,
    pub latency_max_ms: Option<u64>,
    pub latency_distribution_ms: Vec<(u8, u64)>,

    pub metrics: Vec<MetricSeriesSummary>,
}

#[derive(Debug)]
pub struct RunStats {
    requests_total: AtomicU64,
    http_requests_total: AtomicU64,
    grpc_requests_total: AtomicU64,
    iterations_total: AtomicU64,
    dropped_iterations_total: AtomicU64,
    checks_total: AtomicU64,
    checks_failed: AtomicU64,
    checks_by_name: Mutex<HashMap<Arc<str>, Arc<CheckCounters>>>,
    http_errors_total: AtomicU64,
    grpc_errors_total: AtomicU64,
    status_2xx: AtomicU64,
    status_4xx: AtomicU64,
    status_5xx: AtomicU64,
    bytes_received_total: AtomicU64,
    bytes_sent_total: AtomicU64,
    latency_us: Mutex<Histogram<u64>>,
    latency_us_window: Mutex<Histogram<u64>>,

    rps_samples: Mutex<RpsAgg>,

    metrics: Arc<MetricsRegistry>,
    metric_http_reqs: MetricHandle,
    metric_http_req_duration: MetricHandle,
    metric_http_req_failed: MetricHandle,
    metric_grpc_reqs: MetricHandle,
    metric_grpc_req_duration: MetricHandle,
    metric_grpc_req_failed: MetricHandle,
    metric_checks: MetricHandle,
    metric_data_received: MetricHandle,
    metric_data_sent: MetricHandle,
    metric_iterations: MetricHandle,
    metric_iteration_duration: MetricHandle,
}

#[derive(Debug, Clone)]
pub struct LatencySnapshotMs {
    pub mean_ms: f64,
    pub stdev_ms: f64,
    pub max_ms: u64,
    pub p50_ms: u64,
    pub p75_ms: u64,
    pub p90_ms: u64,
    pub p99_ms: u64,
    pub distribution_ms: Vec<(u8, u64)>,
}

impl Default for LatencySnapshotMs {
    fn default() -> Self {
        Self {
            mean_ms: 0.0,
            stdev_ms: 0.0,
            max_ms: 0,
            p50_ms: 0,
            p75_ms: 0,
            p90_ms: 0,
            p99_ms: 0,
            distribution_ms: Vec::new(),
        }
    }
}

#[derive(Debug, Default, Clone, Copy)]
struct RpsAgg {
    count: u64,
    mean: f64,
    m2: f64,
    max: f64,
}

impl RpsAgg {
    fn record(&mut self, sample: f64) {
        if !sample.is_finite() {
            return;
        }

        self.count = self.count.saturating_add(1);
        let delta = sample - self.mean;
        self.mean += delta / (self.count as f64);
        let delta2 = sample - self.mean;
        self.m2 += delta * delta2;
        self.max = self.max.max(sample);
    }

    fn summary(&self) -> (f64, f64, f64, f64) {
        if self.count == 0 {
            return (0.0, 0.0, 0.0, 0.0);
        }

        let avg = self.mean;
        let stdev = if self.count >= 2 {
            (self.m2 / ((self.count - 1) as f64)).sqrt()
        } else {
            0.0
        };

        let stdev_pct = if avg > 0.0 {
            (stdev / avg) * 100.0
        } else {
            0.0
        };
        (avg, stdev, self.max, stdev_pct)
    }
}

impl Default for RunStats {
    fn default() -> Self {
        fn new_hist() -> Histogram<u64> {
            // Track up to 60s in microseconds (with 3 sigfigs).
            Histogram::<u64>::new_with_bounds(1, 60_000_000, 3)
                .unwrap_or_else(|err| panic!("failed to init histogram: {err}"))
        }

        let metrics: Arc<MetricsRegistry> = Arc::new(MetricsRegistry::default());
        let metric_http_reqs = metrics.handle(MetricKind::Counter, "http_reqs");
        let metric_http_req_duration = metrics.handle(MetricKind::Trend, "http_req_duration");
        let metric_http_req_failed = metrics.handle(MetricKind::Rate, "http_req_failed");
        let metric_grpc_reqs = metrics.handle(MetricKind::Counter, "grpc_reqs");
        let metric_grpc_req_duration = metrics.handle(MetricKind::Trend, "grpc_req_duration");
        let metric_grpc_req_failed = metrics.handle(MetricKind::Rate, "grpc_req_failed");
        let metric_checks = metrics.handle(MetricKind::Rate, "checks");
        let metric_data_received = metrics.handle(MetricKind::Counter, "data_received");
        let metric_data_sent = metrics.handle(MetricKind::Counter, "data_sent");
        let metric_iterations = metrics.handle(MetricKind::Counter, "iterations");
        let metric_iteration_duration = metrics.handle(MetricKind::Trend, "iteration_duration");

        Self {
            requests_total: AtomicU64::new(0),
            http_requests_total: AtomicU64::new(0),
            grpc_requests_total: AtomicU64::new(0),
            iterations_total: AtomicU64::new(0),
            dropped_iterations_total: AtomicU64::new(0),
            checks_total: AtomicU64::new(0),
            checks_failed: AtomicU64::new(0),
            checks_by_name: Mutex::new(HashMap::new()),
            http_errors_total: AtomicU64::new(0),
            grpc_errors_total: AtomicU64::new(0),
            status_2xx: AtomicU64::new(0),
            status_4xx: AtomicU64::new(0),
            status_5xx: AtomicU64::new(0),
            bytes_received_total: AtomicU64::new(0),
            bytes_sent_total: AtomicU64::new(0),
            latency_us: Mutex::new(new_hist()),
            latency_us_window: Mutex::new(new_hist()),

            rps_samples: Mutex::new(RpsAgg::default()),

            metrics,
            metric_http_reqs,
            metric_http_req_duration,
            metric_http_req_failed,
            metric_grpc_reqs,
            metric_grpc_req_duration,
            metric_grpc_req_failed,
            metric_checks,
            metric_data_received,
            metric_data_sent,
            metric_iterations,
            metric_iteration_duration,
        }
    }
}

impl RunStats {
    pub fn metric_handle(&self, kind: MetricKind, name: &str) -> MetricHandle {
        self.metrics.handle(kind, name)
    }

    pub fn requests_total(&self) -> u64 {
        self.requests_total.load(Ordering::Relaxed)
    }

    pub fn http_requests_total(&self) -> u64 {
        self.http_requests_total.load(Ordering::Relaxed)
    }

    pub fn grpc_requests_total(&self) -> u64 {
        self.grpc_requests_total.load(Ordering::Relaxed)
    }

    pub fn bytes_received_total(&self) -> u64 {
        self.bytes_received_total.load(Ordering::Relaxed)
    }

    pub fn bytes_sent_total(&self) -> u64 {
        self.bytes_sent_total.load(Ordering::Relaxed)
    }

    pub fn checks_failed_total(&self) -> u64 {
        self.checks_failed.load(Ordering::Relaxed)
    }

    pub fn req_per_sec_summary(&self) -> (f64, f64, f64, f64) {
        let agg = self
            .rps_samples
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        agg.summary()
    }

    pub fn latency_snapshot_ms(&self) -> LatencySnapshotMs {
        let h = self
            .latency_us
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());

        #[allow(clippy::len_zero)]
        if h.len() == 0 {
            return LatencySnapshotMs::default();
        }

        let p50_ms = h.value_at_quantile(0.50) / 1000;
        let p75_ms = h.value_at_quantile(0.75) / 1000;
        let p90_ms = h.value_at_quantile(0.90) / 1000;
        let p99_ms = h.value_at_quantile(0.99) / 1000;

        let mean_ms = h.mean() / 1000.0;
        let stdev_ms = h.stdev() / 1000.0;
        let max_ms = h.max() / 1000;

        let mut dist: Vec<(u8, u64)> = Vec::with_capacity(99);
        for p in 1u8..=99u8 {
            let q = (p as f64) / 100.0;
            let v_ms = h.value_at_quantile(q) / 1000;
            dist.push((p, v_ms));
        }

        LatencySnapshotMs {
            mean_ms,
            stdev_ms,
            max_ms,
            p50_ms,
            p75_ms,
            p90_ms,
            p99_ms,
            distribution_ms: dist,
        }
    }

    pub fn errors_snapshot(&self) -> HashMap<String, u64> {
        let map = self
            .checks_by_name
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());

        let mut out: HashMap<String, u64> = HashMap::new();
        for (name, counters) in map.iter() {
            let failed = counters.failed.load(Ordering::Relaxed);
            if failed == 0 {
                continue;
            }
            out.insert(name.to_string(), failed);
        }
        out
    }

    pub fn failed_requests_total(&self) -> u64 {
        self.http_errors_total.load(Ordering::Relaxed)
            + self.status_4xx.load(Ordering::Relaxed)
            + self.status_5xx.load(Ordering::Relaxed)
            + self.grpc_errors_total.load(Ordering::Relaxed)
    }

    pub fn iterations_total(&self) -> u64 {
        self.iterations_total.load(Ordering::Relaxed)
    }

    pub fn record_iteration(&self, elapsed: Duration) {
        self.iterations_total.fetch_add(1, Ordering::Relaxed);
        self.metric_iterations.add(1.0);

        let ms = elapsed.as_secs_f64() * 1000.0;
        self.metric_iteration_duration.add(ms);
    }

    pub fn record_rps_sample(&self, rps_now: f64) {
        let mut agg = self
            .rps_samples
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        agg.record(rps_now);
    }

    pub fn record_dropped_iterations(&self, n: u64) {
        if n != 0 {
            self.dropped_iterations_total
                .fetch_add(n, Ordering::Relaxed);
        }
    }

    pub fn record_http_status(&self, status: u16) {
        self.requests_total.fetch_add(1, Ordering::Relaxed);
        self.http_requests_total.fetch_add(1, Ordering::Relaxed);
        match status {
            200..=299 => {
                self.status_2xx.fetch_add(1, Ordering::Relaxed);
            }
            400..=499 => {
                self.status_4xx.fetch_add(1, Ordering::Relaxed);
            }
            500..=599 => {
                self.status_5xx.fetch_add(1, Ordering::Relaxed);
            }
            _ => {}
        }
    }

    pub fn record_http_error(&self) {
        self.requests_total.fetch_add(1, Ordering::Relaxed);
        self.http_requests_total.fetch_add(1, Ordering::Relaxed);
        self.http_errors_total.fetch_add(1, Ordering::Relaxed);
    }

    pub fn record_grpc_error(&self) {
        self.requests_total.fetch_add(1, Ordering::Relaxed);
        self.grpc_requests_total.fetch_add(1, Ordering::Relaxed);
        self.grpc_errors_total.fetch_add(1, Ordering::Relaxed);
    }

    fn record_grpc_status(&self, status: u16) {
        self.requests_total.fetch_add(1, Ordering::Relaxed);
        self.grpc_requests_total.fetch_add(1, Ordering::Relaxed);

        if status != 0 {
            self.grpc_errors_total.fetch_add(1, Ordering::Relaxed);
        }
    }

    pub fn record_http_request(&self, req: HttpRequestMeta<'_>, tags: &[(String, String)]) {
        let transport_error = req.transport_error_kind.is_some();

        // Preserve existing aggregated summary behavior.
        if transport_error {
            self.record_http_error();
            let kind = req
                .transport_error_kind
                .map(|k| k.to_string())
                .unwrap_or_else(|| "transport_error".to_string());
            let h = self.check_handle(&format!("http_error:{kind}"));
            self.record_check_handle(&h, false);
        } else if let Some(status) = req.status {
            self.record_http_status(status);
            if status >= 400 {
                let h = self.check_handle(&format!("http_status:{status}"));
                self.record_check_handle(&h, false);
            }
        }
        self.record_latency(req.elapsed);

        if req.bytes_received != 0 {
            self.bytes_received_total
                .fetch_add(req.bytes_received, Ordering::Relaxed);
            self.metric_data_received.add(req.bytes_received as f64);
        }

        if req.bytes_sent != 0 {
            self.bytes_sent_total
                .fetch_add(req.bytes_sent, Ordering::Relaxed);
            self.metric_data_sent.add(req.bytes_sent as f64);
        }

        let duration_ms = req.elapsed.as_secs_f64() * 1000.0;

        let mut merged_tags: Vec<(String, String)> = Vec::with_capacity(tags.len() + 3);
        merged_tags.extend_from_slice(tags);
        merged_tags.push(("method".to_string(), req.method.to_owned()));
        merged_tags.push(("name".to_string(), req.name.to_string()));
        if let Some(status) = req.status {
            merged_tags.push(("status".to_string(), status.to_string()));
        }

        self.metric_http_reqs.add_with_tags(1.0, &merged_tags);
        self.metric_http_req_duration
            .add_with_tags(duration_ms, &merged_tags);

        let failed = transport_error || req.status.is_some_and(|s| s >= 400);
        self.metric_http_req_failed
            .add_bool_with_tags(failed, &merged_tags);
    }

    pub fn record_grpc_request(&self, req: GrpcRequestMeta<'_>, tags: &[(String, String)]) {
        let transport_error = req.transport_error_kind.is_some();

        // Preserve existing aggregated summary behavior (as checks).
        if transport_error {
            self.record_grpc_error();
            let kind = req
                .transport_error_kind
                .map(|k| k.to_string())
                .unwrap_or_else(|| "transport_error".to_string());
            let h = self.check_handle(&format!("grpc_error:{kind}"));
            self.record_check_handle(&h, false);
        } else if let Some(status) = req.status {
            // gRPC: status 0 == OK, anything else is an error.
            self.record_grpc_status(status);
            if status != 0 {
                let h = self.check_handle(&format!("grpc_status:{status}"));
                self.record_check_handle(&h, false);
            }
        }

        self.record_latency(req.elapsed);

        if req.bytes_received != 0 {
            self.bytes_received_total
                .fetch_add(req.bytes_received, Ordering::Relaxed);
            self.metric_data_received.add(req.bytes_received as f64);
        }

        if req.bytes_sent != 0 {
            self.bytes_sent_total
                .fetch_add(req.bytes_sent, Ordering::Relaxed);
            self.metric_data_sent.add(req.bytes_sent as f64);
        }

        let duration_ms = req.elapsed.as_secs_f64() * 1000.0;

        let mut merged_tags: Vec<(String, String)> = Vec::with_capacity(tags.len() + 3);
        merged_tags.extend_from_slice(tags);
        merged_tags.push(("method".to_string(), req.method.to_string()));
        merged_tags.push(("name".to_string(), req.name.to_string()));
        if let Some(status) = req.status {
            merged_tags.push(("status".to_string(), status.to_string()));
        }

        self.metric_grpc_reqs.add_with_tags(1.0, &merged_tags);
        self.metric_grpc_req_duration
            .add_with_tags(duration_ms, &merged_tags);

        let failed = transport_error || req.status.is_some_and(|s| s != 0);
        self.metric_grpc_req_failed
            .add_bool_with_tags(failed, &merged_tags);
    }

    pub fn check_handle(&self, name: &str) -> CheckHandle {
        let counters = {
            let mut map = self
                .checks_by_name
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            if let Some(v) = map.get(name) {
                v.clone()
            } else {
                let key: Arc<str> = Arc::from(name);
                let v = Arc::new(CheckCounters::default());
                map.insert(key, v.clone());
                v
            }
        };

        CheckHandle { counters }
    }

    pub fn record_check_handle(&self, handle: &CheckHandle, ok: bool) {
        self.checks_total.fetch_add(1, Ordering::Relaxed);
        if !ok {
            self.checks_failed.fetch_add(1, Ordering::Relaxed);
        }

        self.metric_checks.add_bool(ok);

        handle.counters.total.fetch_add(1, Ordering::Relaxed);
        if !ok {
            handle.counters.failed.fetch_add(1, Ordering::Relaxed);
        }
    }

    pub fn record_latency(&self, elapsed: Duration) {
        let us = elapsed.as_micros();
        if us == 0 {
            return;
        }

        let value = us as u64;

        {
            let mut h = self
                .latency_us
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            let _ = h.record(value);
        }

        {
            let mut h = self
                .latency_us_window
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            let _ = h.record(value);
        }
    }

    pub fn take_latency_window_ms(&self) -> (Option<f64>, Option<f64>) {
        let mut h = self
            .latency_us_window
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());

        #[allow(clippy::len_zero)]
        let out = if h.len() == 0 {
            (None, None)
        } else {
            let p50 = h.value_at_quantile(0.50) as f64 / 1000.0;
            let p95 = h.value_at_quantile(0.95) as f64 / 1000.0;
            (Some(p50), Some(p95))
        };

        h.reset();
        out
    }

    pub async fn summarize(&self, elapsed: Duration) -> RunSummary {
        let duration_ms = elapsed.as_millis() as u64;
        let secs = elapsed.as_secs_f64().max(1e-9);

        let requests_total = self.requests_total.load(Ordering::Relaxed);
        let dropped_iterations_total = self.dropped_iterations_total.load(Ordering::Relaxed);
        let checks_total = self.checks_total.load(Ordering::Relaxed);
        let checks_failed = self.checks_failed.load(Ordering::Relaxed);
        let bytes_received_total = self.bytes_received_total.load(Ordering::Relaxed);
        let bytes_sent_total = self.bytes_sent_total.load(Ordering::Relaxed);

        let checks_by_name = {
            let map = self
                .checks_by_name
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            let mut out = Vec::with_capacity(map.len());
            for (name, counters) in map.iter() {
                let total = counters.total.load(Ordering::Relaxed);
                let failed = counters.failed.load(Ordering::Relaxed);
                out.push(CheckSummary {
                    name: name.to_string(),
                    total,
                    failed,
                });
            }
            out.sort_by(|a, b| a.name.cmp(&b.name));
            out
        };

        let (p50_ms, p75_ms, p90_ms, p95_ms, p99_ms, mean_ms, stdev_ms, max_ms, dist_ms) = {
            let h = self
                .latency_us
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            #[allow(clippy::len_zero)]
            if h.len() == 0 {
                (None, None, None, None, None, None, None, None, Vec::new())
            } else {
                let p50 = h.value_at_quantile(0.50) as f64 / 1000.0;
                let p75 = h.value_at_quantile(0.75) as f64 / 1000.0;
                let p90 = h.value_at_quantile(0.90) as f64 / 1000.0;
                let p95 = h.value_at_quantile(0.95) as f64 / 1000.0;
                let p99 = h.value_at_quantile(0.99) as f64 / 1000.0;

                let mean = h.mean() / 1000.0;
                let stdev = h.stdev() / 1000.0;
                let max = h.max() / 1000;

                let mut dist: Vec<(u8, u64)> = Vec::with_capacity(99);
                for p in 1u8..=99u8 {
                    let q = (p as f64) / 100.0;
                    let v_ms = h.value_at_quantile(q) / 1000;
                    dist.push((p, v_ms));
                }

                (
                    Some(p50),
                    Some(p75),
                    Some(p90),
                    Some(p95),
                    Some(p99),
                    Some(mean),
                    Some(stdev),
                    Some(max),
                    dist,
                )
            }
        };

        let (req_per_sec_avg, req_per_sec_stdev, req_per_sec_max, req_per_sec_stdev_pct) = {
            let agg = self
                .rps_samples
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            agg.summary()
        };

        let metrics = self.metrics.summarize();

        RunSummary {
            requests_total,
            dropped_iterations_total,
            checks_total,
            checks_failed,
            checks_by_name,
            bytes_received_total,
            bytes_sent_total,
            run_duration_ms: duration_ms,
            rps: (requests_total as f64) / secs,
            req_per_sec_avg,
            req_per_sec_stdev,
            req_per_sec_max,
            req_per_sec_stdev_pct,
            latency_p50_ms: p50_ms,
            latency_p95_ms: p95_ms,
            latency_p75_ms: p75_ms,
            latency_p90_ms: p90_ms,
            latency_p99_ms: p99_ms,
            latency_mean_ms: mean_ms,
            latency_stdev_ms: stdev_ms,
            latency_max_ms: max_ms,
            latency_distribution_ms: dist_ms,

            metrics,
        }
    }
}
