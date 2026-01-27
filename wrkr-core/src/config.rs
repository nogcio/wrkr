use crate::MetricsContext;
use std::time::Duration;

#[derive(Debug, Clone)]
pub struct Stage {
    pub duration: Duration,
    pub target: u64,
}

#[derive(Debug, Clone, Default)]
pub struct RunConfig {
    pub iterations: Option<u64>,
    pub vus: Option<u64>,
    pub duration: Option<Duration>,
}

#[derive(Debug, Clone)]
pub enum ScenarioExecutor {
    ConstantVus {
        vus: u64,
    },

    /// Ramp the number of active VUs up/down over time.
    RampingVus {
        start_vus: u64,
        stages: Vec<Stage>,
    },

    /// Open-model arrival rate (iterations started per `time_unit`), with ramping stages.
    RampingArrivalRate {
        start_rate: u64,
        time_unit: Duration,
        pre_allocated_vus: u64,
        max_vus: u64,
        stages: Vec<Stage>,
    },
}

/// Scenario executor kind (the string form used by scripts/CLI).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, strum::EnumString, strum::Display)]
pub enum ScenarioExecutorKind {
    #[strum(
        serialize = "constant-vus",
        serialize = "constant",
        serialize = "per-vu-iterations"
    )]
    ConstantVus,

    #[strum(serialize = "ramping-vus")]
    RampingVus,

    #[strum(serialize = "ramping-arrival-rate", serialize = "ramping-rps")]
    RampingArrivalRate,
}

impl ScenarioExecutorKind {
    #[must_use]
    pub fn is_ramping(self) -> bool {
        matches!(self, Self::RampingVus | Self::RampingArrivalRate)
    }
}

#[derive(Debug, Clone)]
pub struct ScenarioConfig {
    pub exec: String,
    pub metrics_ctx: MetricsContext,
    pub executor: ScenarioExecutor,
    pub iterations: Option<u64>,
    pub duration: Option<Duration>,
}

#[derive(Debug, Clone, Default)]
pub struct ScriptOptions {
    pub vus: Option<u64>,
    pub iterations: Option<u64>,
    pub duration: Option<Duration>,
    pub scenarios: Vec<ScenarioOptions>,

    /// Threshold assertions.
    pub thresholds: Vec<super::thresholds::ThresholdSet>,
}

#[derive(Debug, Clone)]
pub struct ScenarioOptions {
    pub name: String,
    pub exec: Option<String>,

    /// Scenario-level metric tags (k6-style `Options.scenarios[*].tags`).
    pub tags: Vec<(String, String)>,

    /// Scenario executor. If missing, defaults to constant VUs.
    pub executor: Option<String>,

    pub vus: Option<u64>,
    pub iterations: Option<u64>,
    pub duration: Option<Duration>,

    // Ramping VUs
    pub start_vus: Option<u64>,
    pub stages: Vec<Stage>,

    // Ramping arrival rate
    pub start_rate: Option<u64>,
    pub time_unit: Option<Duration>,
    pub pre_allocated_vus: Option<u64>,
    pub max_vus: Option<u64>,
}
