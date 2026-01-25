use hdrhistogram::Histogram;
use parking_lot::Mutex;
use std::sync::Arc;
use std::sync::atomic::{AtomicI64, AtomicU64, Ordering};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, strum::Display, strum::EnumString)]
pub enum MetricKind {
    Counter,
    Gauge,
    Rate,
    Histogram,
}

#[derive(Debug, Clone)]
pub struct MetricSeriesSummary {
    pub name: String,
    pub kind: MetricKind,
    pub tags: Vec<(String, String)>,
    pub values: MetricValue,
}

#[derive(Debug, Clone)]
pub enum MetricValue {
    Counter(u64),
    Gauge(i64),
    Rate {
        total: u64,
        hits: u64,
        rate: Option<f64>,
    },
    Histogram(HistogramSummary),
}

#[derive(Debug, Clone)]
pub struct HistogramSummary {
    pub p50: Option<f64>,
    pub p75: Option<f64>,
    pub p90: Option<f64>,
    pub p95: Option<f64>,
    pub p99: Option<f64>,
    pub min: Option<f64>,
    pub max: Option<f64>,
    pub mean: Option<f64>,
    pub stdev: Option<f64>,
    pub count: u64,
}

pub(crate) fn new_default_histogram() -> Histogram<u64> {
    // Defaults compatible with typical latency ms.
    match Histogram::<u64>::new_with_bounds(1, 60_000 * 60, 3) {
        Ok(h) => h,
        Err(err) => panic!("failed to create histogram: {err}"),
    }
}

pub(crate) fn summarize_histogram(h: &Histogram<u64>) -> HistogramSummary {
    let count = h.len();
    let map_val = |v| v as f64;

    HistogramSummary {
        p50: (count > 0).then(|| map_val(h.value_at_quantile(0.50))),
        p75: (count > 0).then(|| map_val(h.value_at_quantile(0.75))),
        p90: (count > 0).then(|| map_val(h.value_at_quantile(0.90))),
        p95: (count > 0).then(|| map_val(h.value_at_quantile(0.95))),
        p99: (count > 0).then(|| map_val(h.value_at_quantile(0.99))),
        min: (count > 0).then(|| map_val(h.min())),
        max: (count > 0).then(|| map_val(h.max())),
        mean: (count > 0).then(|| h.mean()),
        stdev: (count > 0).then(|| h.stdev()),
        count,
    }
}

#[derive(Debug)]
pub enum MetricStorage {
    Counter(Arc<AtomicU64>),
    Gauge(Arc<AtomicI64>), // Supports negative values
    Rate(Arc<Rate>),
    Histogram(Arc<Mutex<Histogram<u64>>>),
}

#[derive(Debug)]
pub struct Rate {
    pub total: AtomicU64,
    pub hits: AtomicU64,
}

impl MetricStorage {
    pub fn new(kind: MetricKind) -> Self {
        match kind {
            MetricKind::Counter => MetricStorage::Counter(Arc::new(AtomicU64::new(0))),
            MetricKind::Gauge => MetricStorage::Gauge(Arc::new(AtomicI64::new(0))),
            MetricKind::Rate => MetricStorage::Rate(Arc::new(Rate {
                total: AtomicU64::new(0),
                hits: AtomicU64::new(0),
            })),
            MetricKind::Histogram => {
                MetricStorage::Histogram(Arc::new(Mutex::new(new_default_histogram())))
            }
        }
    }
}

// Public handle for writing metrics
#[derive(Debug, Clone)]
pub enum MetricHandle {
    Counter(Arc<AtomicU64>),
    Gauge(Arc<AtomicI64>),
    Rate(Arc<Rate>),
    Histogram(Arc<Mutex<Histogram<u64>>>),
}

impl MetricHandle {
    #[inline]
    pub fn increment(&self, value: u64) {
        if let MetricHandle::Counter(c) = self {
            c.fetch_add(value, Ordering::Relaxed);
        }
    }

    #[inline]
    pub fn set_gauge(&self, value: i64) {
        if let MetricHandle::Gauge(g) = self {
            g.store(value, Ordering::Relaxed);
        }
    }

    #[inline]
    pub fn increment_gauge(&self, value: i64) {
        if let MetricHandle::Gauge(g) = self {
            g.fetch_add(value, Ordering::Relaxed);
        }
    }

    #[inline]
    pub fn decrement_gauge(&self, value: i64) {
        if let MetricHandle::Gauge(g) = self {
            g.fetch_sub(value, Ordering::Relaxed);
        }
    }

    #[inline]
    pub fn add_rate(&self, hits: u64, total: u64) {
        if let MetricHandle::Rate(r) = self {
            r.hits.fetch_add(hits, Ordering::Relaxed);
            r.total.fetch_add(total, Ordering::Relaxed);
        }
    }

    #[inline]
    pub fn observe_histogram(&self, value: u64) {
        if let MetricHandle::Histogram(h) = self {
            // Locking is unavoidable with shared histogram unless we use a window or thread-local buffer
            // For now, simple mutex
            let mut h = h.lock();
            let _ = h.record(value);
        }
    }
}

impl MetricHandle {
    pub fn get_counter(&self) -> u64 {
        if let MetricHandle::Counter(c) = self {
            c.load(Ordering::Relaxed)
        } else {
            0
        }
    }

    pub fn get_gauge(&self) -> i64 {
        if let MetricHandle::Gauge(g) = self {
            g.load(Ordering::Relaxed)
        } else {
            0
        }
    }

    pub fn get_rate(&self) -> (u64, u64) {
        if let MetricHandle::Rate(r) = self {
            (
                r.total.load(Ordering::Relaxed),
                r.hits.load(Ordering::Relaxed),
            )
        } else {
            (0, 0)
        }
    }
}
