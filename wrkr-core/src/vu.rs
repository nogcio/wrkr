use std::sync::Arc;
use std::sync::Mutex;
use std::sync::OnceLock;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Instant;
use tokio::sync::Barrier;
use tokio::sync::Notify;

use wrkr_metrics::{MetricHandle, MetricKind};

use super::gate::IterationGate;
use super::metrics_context::MetricsContext;
use super::pacer::ArrivalPacer;
use super::run::RunScenariosContext;
use super::schedule::RampingU64Schedule;

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

impl Default for StartSignal {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Clone)]
pub struct VuContext {
    pub vu_id: u64,
    /// Total VUs spawned for the run (across all scenarios).
    pub max_vus: u64,
    pub metrics_ctx: MetricsContext,
    pub scenario_vu: u64,
    pub exec: String,
    pub work: VuWork,

    pub run_ctx: Arc<RunScenariosContext>,

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
        let handle = self
            .run_ctx
            .metrics
            .register("vu_active", MetricKind::Gauge);

        // Track the peak active VUs during the run.
        // This avoids a confusing `vu_active = 0` in end-of-run summaries.
        let peak_handle = self
            .run_ctx
            .metrics
            .register("vu_active_max", MetricKind::Gauge);
        let tags = self
            .run_ctx
            .metrics
            .resolve_tags(&[("scenario", self.metrics_ctx.scenario())]);

        if let Some(MetricHandle::Gauge(g)) = self.run_ctx.metrics.get_handle(handle, tags.clone())
        {
            let new_active = g.fetch_add(1, Ordering::Relaxed).saturating_add(1);

            if let Some(MetricHandle::Gauge(peak)) =
                self.run_ctx.metrics.get_handle(peak_handle, tags.clone())
            {
                // CAS loop to keep the max without races.
                let mut cur = peak.load(Ordering::Relaxed);
                while new_active > cur {
                    match peak.compare_exchange_weak(
                        cur,
                        new_active,
                        Ordering::Relaxed,
                        Ordering::Relaxed,
                    ) {
                        Ok(_) => break,
                        Err(observed) => cur = observed,
                    }
                }
            }
        }

        ActiveVuGuard {
            metrics: self.run_ctx.metrics.clone(),
            handle,
            tags,
        }
    }

    pub fn record_iteration(&self, duration: std::time::Duration, success: bool) {
        let extra_tags = self
            .metrics_ctx
            .scenario_tag_refs(&["scenario", "status", "group"]);

        self.run_ctx.iteration_metrics.record_iteration(
            &self.run_ctx.metrics,
            crate::IterationSample {
                scenario: self.metrics_ctx.scenario(),
                success,
                duration,
            },
            &extra_tags,
        );
    }
}
