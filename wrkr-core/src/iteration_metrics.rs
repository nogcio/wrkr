use std::sync::atomic::Ordering;

use wrkr_metrics::{MetricHandle, MetricId, MetricKind, Registry};

#[derive(Debug, Clone, Copy)]
pub struct IterationMetricIds {
    pub iterations_total: MetricId,
    /// Iteration duration in microseconds.
    pub iteration_duration_seconds: MetricId,
}

#[derive(Debug, Clone, Copy)]
pub struct IterationSample<'a> {
    pub scenario: &'a str,
    pub success: bool,
    pub duration: std::time::Duration,
}

impl IterationMetricIds {
    pub fn register(metrics: &Registry) -> Self {
        Self {
            iterations_total: metrics.register("iterations_total", MetricKind::Counter),
            iteration_duration_seconds: metrics
                .register("iteration_duration_seconds", MetricKind::Histogram),
        }
    }

    pub fn record_iteration(
        &self,
        metrics: &Registry,
        sample: IterationSample<'_>,
        extra_tags: &[(&str, &str)],
    ) {
        let status = if sample.success { "success" } else { "failure" };

        let filter_extra = |(k, _v): &(&str, &str)| !matches!(*k, "scenario" | "status");

        let resolve = |base: &[(&str, &str)]| {
            if extra_tags.is_empty() {
                return metrics.resolve_tags(base);
            }

            let mut merged: Vec<(&str, &str)> = Vec::with_capacity(base.len() + extra_tags.len());
            merged.extend_from_slice(base);
            merged.extend(extra_tags.iter().copied().filter(filter_extra));
            metrics.resolve_tags(&merged)
        };

        let tags = resolve(&[("scenario", sample.scenario), ("status", status)]);

        if let Some(MetricHandle::Counter(c)) =
            metrics.get_handle(self.iterations_total, tags.clone())
        {
            c.fetch_add(1, Ordering::Relaxed);
        }

        if let Some(MetricHandle::Histogram(h)) =
            metrics.get_handle(self.iteration_duration_seconds, tags)
        {
            let duration_us: u64 = sample.duration.as_micros().try_into().unwrap_or(u64::MAX);
            let _ = h.lock().record(duration_us.max(1));
        }
    }
}
