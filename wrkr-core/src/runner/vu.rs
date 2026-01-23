use crate::HttpClient;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::OnceLock;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Instant;
use tokio::sync::Barrier;
use tokio::sync::Notify;

use super::gate::IterationGate;
use super::pacer::ArrivalPacer;
use super::schedule::RampingU64Schedule;
use super::shared::SharedStore;
use super::stats::RunStats;

pub type EnvVars = Arc<[(Arc<str>, Arc<str>)]>;

#[derive(Debug)]
pub struct StartSignal {
    started: AtomicBool,
    notify: Notify,
}

impl StartSignal {
    pub fn new() -> Self {
        Self {
            started: AtomicBool::new(false),
            notify: Notify::new(),
        }
    }

    pub fn start(&self) {
        self.started.store(true, Ordering::Release);
        self.notify.notify_waiters();
    }

    pub async fn wait(&self) {
        while !self.started.load(Ordering::Acquire) {
            self.notify.notified().await;
        }
    }
}

#[derive(Debug, Clone)]
pub struct VuContext {
    pub vu_id: u64,
    /// Total VUs spawned for the run (across all scenarios).
    pub max_vus: u64,
    pub scenario: Arc<str>,
    pub scenario_vu: u64,
    pub script: Arc<str>,
    pub script_path: Option<Arc<PathBuf>>,
    pub exec: Arc<str>,
    pub client: Arc<HttpClient>,
    pub stats: Arc<RunStats>,
    pub work: VuWork,
    pub env: EnvVars,

    /// Process-wide shared storage for coordinating VUs.
    pub shared: Arc<SharedStore>,

    pub run_started: Arc<OnceLock<Instant>>,

    pub init_error: Arc<Mutex<Option<String>>>,
    pub ready_barrier: Arc<Barrier>,
    pub start_signal: Arc<StartSignal>,
}

#[derive(Debug, Clone)]
pub enum VuWork {
    Constant {
        gate: Arc<IterationGate>,
    },
    RampingVus {
        schedule: Arc<RampingU64Schedule>,
    },
    RampingArrivalRate {
        schedule: Arc<RampingU64Schedule>,
        time_unit: std::time::Duration,
        pacer: Arc<ArrivalPacer>,
    },
}
