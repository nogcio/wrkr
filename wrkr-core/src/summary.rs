use std::collections::HashMap;

#[derive(Debug, Default, Clone)]
pub struct RunSummary {
    pub scenarios: Vec<ScenarioSummary>,
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

    pub latency_ms: Option<wrkr_metrics::HistogramSummary>,
}
