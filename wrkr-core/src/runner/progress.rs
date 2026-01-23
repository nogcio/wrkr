use std::collections::HashMap;
use std::time::Duration;

#[derive(Debug, Clone)]
pub struct LiveMetrics {
    /// Requests/sec observed during the last progress interval.
    pub rps_now: f64,

    /// Bytes received/sec observed during the last progress interval.
    pub bytes_received_per_sec_now: u64,

    /// Bytes sent/sec observed during the last progress interval.
    pub bytes_sent_per_sec_now: u64,

    /// Total requests observed so far.
    pub requests_total: u64,

    /// Total bytes received observed so far.
    pub bytes_received_total: u64,

    /// Total bytes sent observed so far.
    pub bytes_sent_total: u64,

    /// Total failed requests observed so far.
    pub failed_requests_total: u64,

    /// Total failed checks observed so far.
    pub checks_failed_total: u64,

    /// Aggregate requests/sec statistics across progress intervals.
    pub req_per_sec_avg: f64,
    pub req_per_sec_stdev: f64,
    pub req_per_sec_max: f64,
    pub req_per_sec_stdev_pct: f64,

    /// Aggregate latency stats (milliseconds) across the whole run so far.
    pub latency_mean_ms: f64,
    pub latency_stdev_ms: f64,
    pub latency_max_ms: u64,
    pub latency_p50_ms: u64,
    pub latency_p75_ms: u64,
    pub latency_p90_ms: u64,
    pub latency_p99_ms: u64,
    pub latency_stdev_pct: f64,

    /// Percentiles 1..=99, values in milliseconds.
    pub latency_distribution_ms: Vec<(u8, u64)>,

    /// Failed checks breakdown by name.
    pub checks_failed: HashMap<String, u64>,
    pub latency_p50_ms_now: Option<f64>,
    pub latency_p90_ms_now: Option<f64>,
    pub latency_p95_ms_now: Option<f64>,
    pub latency_p99_ms_now: Option<f64>,
    /// Failed requests/sec observed during the last progress interval.
    pub failed_rps_now: f64,
    /// Failed requests / total requests observed during the last progress interval (0..=1).
    pub error_rate_now: f64,
    /// Error breakdown during the last progress interval, keyed by status/code.
    pub errors_now: HashMap<String, u64>,
    pub iterations_total: u64,
    pub iterations_per_sec_now: f64,
}

#[derive(Debug, Clone)]
pub struct StageProgress {
    /// 1-based stage index.
    pub stage: usize,
    pub stages: usize,
    pub stage_elapsed: Duration,
    pub stage_remaining: Duration,
    pub start_target: u64,
    pub end_target: u64,
    pub current_target: u64,
}

#[derive(Debug, Clone)]
pub enum ScenarioProgress {
    ConstantVus {
        vus: u64,
        duration: Option<Duration>,
    },
    RampingVus {
        total_duration: Duration,
        stage: Option<StageProgress>,
    },
    RampingArrivalRate {
        time_unit: Duration,
        total_duration: Duration,
        stage: Option<StageProgress>,
        active_vus: u64,
        max_vus: u64,
        dropped_iterations_total: u64,
    },
}

#[derive(Debug, Clone)]
pub struct ProgressUpdate {
    /// Monotonic tick counter (1-based) for progress emissions.
    pub tick: u64,
    pub elapsed: Duration,
    pub scenario: String,
    pub exec: String,
    pub metrics: LiveMetrics,
    pub progress: ScenarioProgress,
}

pub type ProgressFn = std::sync::Arc<dyn Fn(ProgressUpdate) + Send + Sync + 'static>;
