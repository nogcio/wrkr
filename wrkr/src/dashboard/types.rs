use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct DashboardPoint {
    pub tick: u64,
    pub elapsed_seconds: f64,
    pub interval_seconds: f64,

    pub vus: u64,
    pub vus_max: u64,

    pub requests_per_sec: f64,
    pub iterations_per_sec: f64,

    pub bytes_received_per_sec: u64,
    pub bytes_sent_per_sec: u64,

    pub requests_total: u64,
    pub failed_requests_total: u64,
    pub iterations_total: u64,

    pub bytes_received_total: u64,
    pub bytes_sent_total: u64,

    pub checks_failed_total: u64,

    /// Failed requests / total requests for the last interval.
    pub error_rate: f64,

    /// HTTP request duration (ms), cumulative so far.
    pub http_req_duration_avg_ms: Option<f64>,
    pub http_req_duration_p90_ms: Option<f64>,
    pub http_req_duration_p95_ms: Option<f64>,
    pub http_req_duration_p99_ms: Option<f64>,

    /// Iteration duration (ms), cumulative so far.
    pub iteration_duration_avg_ms: Option<f64>,
    pub iteration_duration_p90_ms: Option<f64>,
    pub iteration_duration_p95_ms: Option<f64>,
    pub iteration_duration_p99_ms: Option<f64>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct DashboardSnapshot {
    pub schema: &'static str,
    pub points: Vec<DashboardPoint>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub(crate) enum DashboardEvent {
    Snapshot { snapshot: DashboardSnapshot },
    Tick { point: Box<DashboardPoint> },
    Done,
}
