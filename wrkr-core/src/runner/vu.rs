use crate::HttpClient;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::OnceLock;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Instant;
use tokio::sync::Barrier;
use tokio::sync::Notify;

use wrkr_metrics::{MetricHandle, MetricKind};

use super::gate::IterationGate;
use super::pacer::ArrivalPacer;
use super::schedule::RampingU64Schedule;
use super::shared::SharedStore;

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
    pub work: VuWork,
    pub env: EnvVars,

    /// Process-wide shared storage for coordinating VUs.
    pub shared: Arc<SharedStore>,

    pub metrics: Arc<wrkr_metrics::Registry>,

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

pub struct ActiveVuGuard {
    metrics: Arc<wrkr_metrics::Registry>,
    handle: wrkr_metrics::MetricId,
    tags: wrkr_metrics::TagSet,
}

impl Drop for ActiveVuGuard {
    fn drop(&mut self) {
        if let Some(MetricHandle::Gauge(g)) =
            self.metrics.get_handle(self.handle, self.tags.clone())
        {
            g.fetch_sub(1, Ordering::Relaxed);
        }
    }
}

impl VuContext {
    pub fn enter_active_vu(&self) -> ActiveVuGuard {
        let handle = self.metrics.register("vu_active", MetricKind::Gauge);
        let tags = self.metrics.resolve_tags(&[("scenario", &self.scenario)]);

        if let Some(MetricHandle::Gauge(g)) = self.metrics.get_handle(handle, tags.clone()) {
            g.fetch_add(1, Ordering::Relaxed);
        }

        ActiveVuGuard {
            metrics: self.metrics.clone(),
            handle,
            tags,
        }
    }

    pub fn record_iteration(&self, duration: std::time::Duration, success: bool) {
        let m_iterations = self
            .metrics
            .register("iterations_total", MetricKind::Counter);
        let m_duration = self
            .metrics
            .register("iteration_duration_seconds", MetricKind::Histogram);

        let status = if success { "success" } else { "failure" };
        let tags = self
            .metrics
            .resolve_tags(&[("scenario", &self.scenario), ("status", status)]);

        if let Some(MetricHandle::Counter(c)) = self.metrics.get_handle(m_iterations, tags.clone())
        {
            c.fetch_add(1, Ordering::Relaxed);
        }

        // Timer usually doesn't need status, or maybe it does?
        // k6: http_req_duration has status. Iteration duration usually doesn't have status tag in some systems, but does in others.
        // Let's keep status on duration for now if we want.
        // Actually iteration_duration usually tracks *successful* and *failed* iterations separately?
        // Let's use the same tags for both for consistency.

        if let Some(MetricHandle::Histogram(h)) = self.metrics.get_handle(m_duration, tags) {
            h.lock().record(duration.as_micros() as u64).unwrap_or(());
        }
    }
}
