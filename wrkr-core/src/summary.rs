use std::collections::HashMap;

use crate::ThresholdViolation;

#[derive(Debug, Default, Clone)]
pub struct RunSummary {
    pub scenarios: Vec<ScenarioSummary>,

    /// Full metric series summary snapshot at end of run.
    pub metrics: Vec<wrkr_metrics::MetricSeriesSummary>,

    /// Threshold violations computed from `metrics` and the configured threshold sets.
    pub threshold_violations: Vec<ThresholdViolation>,
}

#[derive(Debug, Default, Clone)]
pub struct ScenarioSummary {
    pub scenario: String,

    pub requests_total: u64,
    pub failed_requests_total: u64,
    pub bytes_received_total: u64,
    pub bytes_sent_total: u64,
    pub iterations_total: u64,

    pub checks_failed_total: u64,
    pub checks_failed: HashMap<String, u64>,

    pub latency: Option<wrkr_metrics::HistogramSummary>,
}
