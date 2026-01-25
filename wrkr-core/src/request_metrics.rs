use std::sync::atomic::Ordering;

use wrkr_metrics::{MetricHandle, MetricId, MetricKind, Registry};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, strum::Display, strum::EnumString)]
#[strum(serialize_all = "snake_case")]
pub enum Protocol {
    Http,
    Grpc,
}

#[derive(Debug, Clone, Copy)]
pub struct RequestMetricIds {
    pub requests_total: MetricId,
    pub bytes_received_total: MetricId,
    pub bytes_sent_total: MetricId,
    pub errors_total: MetricId,
    pub errors_by_kind_total: MetricId,
    /// Request latency in milliseconds.
    pub latency_ms: MetricId,
}

#[derive(Debug, Clone, Copy)]
pub struct RequestSample<'a> {
    pub scenario: &'a str,
    pub protocol: Protocol,
    /// Whether the transport succeeded.
    pub ok: bool,
    pub latency: std::time::Duration,
    pub bytes_received: u64,
    pub bytes_sent: u64,
    pub error_kind: Option<&'a str>,
}

impl RequestMetricIds {
    pub fn register(metrics: &Registry) -> Self {
        Self {
            requests_total: metrics.register("requests_total", MetricKind::Counter),
            bytes_received_total: metrics.register("bytes_received_total", MetricKind::Counter),
            bytes_sent_total: metrics.register("bytes_sent_total", MetricKind::Counter),
            errors_total: metrics.register("request_errors_total", MetricKind::Counter),
            errors_by_kind_total: metrics
                .register("request_errors_by_kind_total", MetricKind::Counter),
            latency_ms: metrics.register("request_latency_ms", MetricKind::Histogram),
        }
    }

    pub fn record_request(
        &self,
        metrics: &Registry,
        sample: RequestSample<'_>,
        extra_tags: &[(&str, &str)],
    ) {
        let protocol_str = sample.protocol.to_string();

        let filter_extra =
            |(k, _v): &(&str, &str)| !matches!(*k, "scenario" | "protocol" | "error_kind");

        let resolve = |base: &[(&str, &str)]| {
            if extra_tags.is_empty() {
                return metrics.resolve_tags(base);
            }

            let mut merged: Vec<(&str, &str)> = Vec::with_capacity(base.len() + extra_tags.len());
            merged.extend_from_slice(base);
            merged.extend(extra_tags.iter().copied().filter(filter_extra));
            metrics.resolve_tags(&merged)
        };

        // Counters (protocol-scoped)
        let tags = resolve(&[
            ("scenario", sample.scenario),
            ("protocol", protocol_str.as_str()),
        ]);

        if let Some(MetricHandle::Counter(c)) =
            metrics.get_handle(self.requests_total, tags.clone())
        {
            c.fetch_add(1, Ordering::Relaxed);
        }

        if let Some(MetricHandle::Counter(c)) =
            metrics.get_handle(self.bytes_received_total, tags.clone())
        {
            c.fetch_add(sample.bytes_received, Ordering::Relaxed);
        }

        if let Some(MetricHandle::Counter(c)) = metrics.get_handle(self.bytes_sent_total, tags) {
            c.fetch_add(sample.bytes_sent, Ordering::Relaxed);
        }

        // Errors (two series: total + by-kind)
        if !sample.ok {
            let tags = resolve(&[
                ("scenario", sample.scenario),
                ("protocol", protocol_str.as_str()),
            ]);
            if let Some(MetricHandle::Counter(c)) = metrics.get_handle(self.errors_total, tags) {
                c.fetch_add(1, Ordering::Relaxed);
            }

            if let Some(kind) = sample.error_kind {
                let tags = resolve(&[
                    ("scenario", sample.scenario),
                    ("protocol", protocol_str.as_str()),
                    ("error_kind", kind),
                ]);
                if let Some(MetricHandle::Counter(c)) =
                    metrics.get_handle(self.errors_by_kind_total, tags)
                {
                    c.fetch_add(1, Ordering::Relaxed);
                }
            }
        }

        // Latency histogram (two series: overall + protocol-scoped)
        let latency_ms: u64 = sample.latency.as_millis().try_into().unwrap_or(u64::MAX);

        let overall_tags = resolve(&[("scenario", sample.scenario)]);
        if let Some(MetricHandle::Histogram(h)) = metrics.get_handle(self.latency_ms, overall_tags)
        {
            let mut h = h.lock();
            let _ = h.record(latency_ms.max(1));
        }

        let tags = resolve(&[
            ("scenario", sample.scenario),
            ("protocol", protocol_str.as_str()),
        ]);
        if let Some(MetricHandle::Histogram(h)) = metrics.get_handle(self.latency_ms, tags) {
            let mut h = h.lock();
            let _ = h.record(latency_ms.max(1));
        }
    }
}
