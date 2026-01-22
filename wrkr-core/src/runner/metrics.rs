use hdrhistogram::Histogram;
use std::collections::HashMap;
use std::hash::{Hash, Hasher};
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::atomic::{AtomicU64, Ordering};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MetricKind {
    Trend,
    Counter,
    Gauge,
    Rate,
}

#[derive(Debug, Clone)]
pub struct MetricSeriesSummary {
    pub name: String,
    pub kind: MetricKind,
    pub tags: Vec<(String, String)>,
    pub values: MetricValues,
}

#[derive(Debug, Clone)]
pub enum MetricValues {
    Trend {
        count: u64,
        min: Option<f64>,
        max: Option<f64>,
        avg: Option<f64>,
        p50: Option<f64>,
        p90: Option<f64>,
        p95: Option<f64>,
        p99: Option<f64>,
    },
    Counter {
        value: f64,
    },
    Gauge {
        value: f64,
    },
    Rate {
        total: u64,
        trues: u64,
        rate: Option<f64>,
    },
}

#[derive(Debug, Clone)]
pub struct MetricHandle {
    registry: Arc<MetricsRegistry>,
    base: Arc<Metric>,
}

impl MetricHandle {
    pub fn add(&self, value: f64) {
        self.base.add(value);
    }

    pub fn add_with_tags(&self, value: f64, tags: &[(String, String)]) {
        self.base.add(value);
        if tags.is_empty() {
            return;
        }
        self.registry
            .series(self.base.kind, &self.base.name, tags)
            .add(value);
    }

    pub fn add_bool(&self, value: bool) {
        self.base.add_bool(value);
    }

    pub fn add_bool_with_tags(&self, value: bool, tags: &[(String, String)]) {
        self.base.add_bool(value);
        if tags.is_empty() {
            return;
        }
        self.registry
            .series(self.base.kind, &self.base.name, tags)
            .add_bool(value);
    }

    pub fn kind(&self) -> MetricKind {
        self.base.kind
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct TagSet(Arc<[(Arc<str>, Arc<str>)]>);

impl Hash for TagSet {
    fn hash<H: Hasher>(&self, state: &mut H) {
        for (k, v) in self.0.iter() {
            k.hash(state);
            v.hash(state);
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct MetricKey {
    kind: MetricKind,
    name: Arc<str>,
    tags: TagSet,
}

fn normalize_tags(tags: &[(String, String)]) -> TagSet {
    if tags.is_empty() {
        return TagSet(Arc::from([]));
    }

    let mut v: Vec<(Arc<str>, Arc<str>)> = tags
        .iter()
        .map(|(k, v)| (Arc::<str>::from(k.as_str()), Arc::<str>::from(v.as_str())))
        .collect();
    v.sort_by(|a, b| a.0.cmp(&b.0).then_with(|| a.1.cmp(&b.1)));
    TagSet(Arc::from(v.into_boxed_slice()))
}

#[derive(Debug, Default)]
pub struct MetricsRegistry {
    series: Mutex<HashMap<MetricKey, Arc<Metric>>>,
}

impl MetricsRegistry {
    pub fn handle(self: &Arc<Self>, kind: MetricKind, name: &str) -> MetricHandle {
        let base = self.series(kind, name, &[]);
        MetricHandle {
            registry: self.clone(),
            base,
        }
    }

    pub fn series(
        self: &Arc<Self>,
        kind: MetricKind,
        name: &str,
        tags: &[(String, String)],
    ) -> Arc<Metric> {
        let name: Arc<str> = Arc::from(name);
        let tags = normalize_tags(tags);
        let key = MetricKey {
            kind,
            name: name.clone(),
            tags: tags.clone(),
        };

        let mut map = self.series.lock().unwrap_or_else(|p| p.into_inner());
        if let Some(existing) = map.get(&key) {
            return existing.clone();
        }

        let metric = Arc::new(Metric::new(kind, name.clone(), tags));
        map.insert(key, metric.clone());
        metric
    }

    pub fn summarize(&self) -> Vec<MetricSeriesSummary> {
        let map = self.series.lock().unwrap_or_else(|p| p.into_inner());
        let mut out = Vec::with_capacity(map.len());
        for metric in map.values() {
            out.push(metric.summarize());
        }
        out.sort_by(|a, b| a.name.cmp(&b.name).then_with(|| a.tags.cmp(&b.tags)));
        out
    }
}

#[derive(Debug)]
struct TrendAgg {
    count: AtomicU64,
    sum_scaled: AtomicU64,
    min_scaled: AtomicU64,
    max_scaled: AtomicU64,
    hist: Mutex<Histogram<u64>>,
}

impl TrendAgg {
    fn new() -> Self {
        let hist = Histogram::<u64>::new_with_bounds(1, 60_000_000_000, 3)
            .unwrap_or_else(|err| panic!("failed to init histogram: {err}"));
        Self {
            count: AtomicU64::new(0),
            sum_scaled: AtomicU64::new(0),
            min_scaled: AtomicU64::new(u64::MAX),
            max_scaled: AtomicU64::new(0),
            hist: Mutex::new(hist),
        }
    }

    fn record(&self, value: f64) {
        if !value.is_finite() {
            return;
        }
        let scaled = (value * 1000.0).round();
        if scaled <= 0.0 {
            return;
        }
        let scaled = scaled as u64;

        self.count.fetch_add(1, Ordering::Relaxed);
        self.sum_scaled.fetch_add(scaled, Ordering::Relaxed);

        let mut cur = self.min_scaled.load(Ordering::Relaxed);
        while scaled < cur {
            match self.min_scaled.compare_exchange_weak(
                cur,
                scaled,
                Ordering::Relaxed,
                Ordering::Relaxed,
            ) {
                Ok(_) => break,
                Err(v) => cur = v,
            }
        }

        let mut cur = self.max_scaled.load(Ordering::Relaxed);
        while scaled > cur {
            match self.max_scaled.compare_exchange_weak(
                cur,
                scaled,
                Ordering::Relaxed,
                Ordering::Relaxed,
            ) {
                Ok(_) => break,
                Err(v) => cur = v,
            }
        }

        let mut h = self.hist.lock().unwrap_or_else(|p| p.into_inner());
        let _ = h.record(scaled);
    }

    fn summarize(&self) -> MetricValues {
        let count = self.count.load(Ordering::Relaxed);
        if count == 0 {
            return MetricValues::Trend {
                count: 0,
                min: None,
                max: None,
                avg: None,
                p50: None,
                p90: None,
                p95: None,
                p99: None,
            };
        }

        let sum = self.sum_scaled.load(Ordering::Relaxed) as f64;
        let min = self.min_scaled.load(Ordering::Relaxed);
        let max = self.max_scaled.load(Ordering::Relaxed);

        let h = self.hist.lock().unwrap_or_else(|p| p.into_inner());
        #[allow(clippy::len_zero)]
        let (p50, p90, p95, p99) = if h.len() == 0 {
            (None, None, None, None)
        } else {
            (
                Some(h.value_at_quantile(0.50) as f64 / 1000.0),
                Some(h.value_at_quantile(0.90) as f64 / 1000.0),
                Some(h.value_at_quantile(0.95) as f64 / 1000.0),
                Some(h.value_at_quantile(0.99) as f64 / 1000.0),
            )
        };

        MetricValues::Trend {
            count,
            min: Some(min as f64 / 1000.0),
            max: Some(max as f64 / 1000.0),
            avg: Some(sum / (count as f64) / 1000.0),
            p50,
            p90,
            p95,
            p99,
        }
    }
}

#[derive(Debug, Default)]
struct ScalarAgg {
    value: Mutex<f64>,
}

impl ScalarAgg {
    fn add(&self, v: f64) {
        if !v.is_finite() {
            return;
        }
        let mut guard = self.value.lock().unwrap_or_else(|p| p.into_inner());
        *guard += v;
    }

    fn set(&self, v: f64) {
        if !v.is_finite() {
            return;
        }
        let mut guard = self.value.lock().unwrap_or_else(|p| p.into_inner());
        *guard = v;
    }

    fn get(&self) -> f64 {
        *self.value.lock().unwrap_or_else(|p| p.into_inner())
    }
}

#[derive(Debug, Default)]
struct RateAgg {
    total: AtomicU64,
    trues: AtomicU64,
}

impl RateAgg {
    fn add(&self, v: bool) {
        self.total.fetch_add(1, Ordering::Relaxed);
        if v {
            self.trues.fetch_add(1, Ordering::Relaxed);
        }
    }

    fn summarize(&self) -> MetricValues {
        let total = self.total.load(Ordering::Relaxed);
        let trues = self.trues.load(Ordering::Relaxed);
        let rate = if total == 0 {
            None
        } else {
            Some(trues as f64 / total as f64)
        };
        MetricValues::Rate { total, trues, rate }
    }
}

#[derive(Debug)]
pub struct Metric {
    kind: MetricKind,
    name: Arc<str>,
    tags: TagSet,
    trend: Option<TrendAgg>,
    counter: Option<ScalarAgg>,
    gauge: Option<ScalarAgg>,
    rate: Option<RateAgg>,
}

impl Metric {
    fn new(kind: MetricKind, name: Arc<str>, tags: TagSet) -> Self {
        match kind {
            MetricKind::Trend => Self {
                kind,
                name,
                tags,
                trend: Some(TrendAgg::new()),
                counter: None,
                gauge: None,
                rate: None,
            },
            MetricKind::Counter => Self {
                kind,
                name,
                tags,
                trend: None,
                counter: Some(ScalarAgg::default()),
                gauge: None,
                rate: None,
            },
            MetricKind::Gauge => Self {
                kind,
                name,
                tags,
                trend: None,
                counter: None,
                gauge: Some(ScalarAgg::default()),
                rate: None,
            },
            MetricKind::Rate => Self {
                kind,
                name,
                tags,
                trend: None,
                counter: None,
                gauge: None,
                rate: Some(RateAgg::default()),
            },
        }
    }

    pub(crate) fn add(&self, value: f64) {
        match self.kind {
            MetricKind::Trend => {
                if let Some(t) = &self.trend {
                    t.record(value);
                }
            }
            MetricKind::Counter => {
                if let Some(c) = &self.counter {
                    c.add(value);
                }
            }
            MetricKind::Gauge => {
                if let Some(g) = &self.gauge {
                    g.set(value);
                }
            }
            MetricKind::Rate => {
                // ignore; use add_bool
            }
        }
    }

    pub(crate) fn add_bool(&self, value: bool) {
        if self.kind != MetricKind::Rate {
            return;
        }
        if let Some(r) = &self.rate {
            r.add(value);
        }
    }

    fn summarize(&self) -> MetricSeriesSummary {
        let tags: Vec<(String, String)> = self
            .tags
            .0
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect();

        let values = match self.kind {
            MetricKind::Trend => {
                self.trend
                    .as_ref()
                    .map(TrendAgg::summarize)
                    .unwrap_or(MetricValues::Trend {
                        count: 0,
                        min: None,
                        max: None,
                        avg: None,
                        p50: None,
                        p90: None,
                        p95: None,
                        p99: None,
                    })
            }
            MetricKind::Counter => MetricValues::Counter {
                value: self.counter.as_ref().map(ScalarAgg::get).unwrap_or(0.0),
            },
            MetricKind::Gauge => MetricValues::Gauge {
                value: self.gauge.as_ref().map(ScalarAgg::get).unwrap_or(0.0),
            },
            MetricKind::Rate => {
                self.rate
                    .as_ref()
                    .map(RateAgg::summarize)
                    .unwrap_or(MetricValues::Rate {
                        total: 0,
                        trues: 0,
                        rate: None,
                    })
            }
        };

        MetricSeriesSummary {
            name: self.name.to_string(),
            kind: self.kind,
            tags,
            values,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn series_tag_order_is_normalized() {
        let metrics = Arc::new(MetricsRegistry::default());

        let a = metrics.series(
            MetricKind::Counter,
            "m",
            &[
                ("b".to_string(), "2".to_string()),
                ("a".to_string(), "1".to_string()),
            ],
        );
        let b = metrics.series(
            MetricKind::Counter,
            "m",
            &[
                ("a".to_string(), "1".to_string()),
                ("b".to_string(), "2".to_string()),
            ],
        );

        // Same logical tagset should point at the same underlying series.
        assert!(Arc::ptr_eq(&a, &b));

        a.add(1.0);
        let summary = metrics.summarize();
        let s = summary
            .iter()
            .find(|s| s.name == "m" && s.kind == MetricKind::Counter)
            .unwrap_or_else(|| panic!("missing metric summary"));

        assert_eq!(
            s.tags,
            vec![
                ("a".to_string(), "1".to_string()),
                ("b".to_string(), "2".to_string())
            ]
        );
    }

    #[test]
    fn trend_ignores_non_positive_and_non_finite_values() {
        let metrics = Arc::new(MetricsRegistry::default());
        let h = metrics.handle(MetricKind::Trend, "t");

        h.add(f64::NAN);
        h.add(0.0);
        h.add(-1.0);
        h.add(1.0);
        h.add(2.0);

        let summary = metrics.summarize();
        let s = summary
            .iter()
            .find(|s| s.name == "t" && s.tags.is_empty())
            .unwrap_or_else(|| panic!("missing trend summary"));

        let MetricValues::Trend {
            count,
            min,
            max,
            avg,
            ..
        } = &s.values
        else {
            panic!("expected trend values");
        };

        assert_eq!(*count, 2);
        assert_eq!(*min, Some(1.0));
        assert_eq!(*max, Some(2.0));
        assert_eq!(*avg, Some(1.5));
    }

    #[test]
    fn rate_records_total_and_trues() {
        let metrics = Arc::new(MetricsRegistry::default());
        let h = metrics.handle(MetricKind::Rate, "r");

        h.add_bool(true);
        h.add_bool(false);
        h.add_bool(true);

        let summary = metrics.summarize();
        let s = summary
            .iter()
            .find(|s| s.name == "r" && s.tags.is_empty())
            .unwrap_or_else(|| panic!("missing rate summary"));

        let MetricValues::Rate { total, trues, rate } = &s.values else {
            panic!("expected rate values");
        };

        assert_eq!(*total, 3);
        assert_eq!(*trues, 2);
        assert_eq!(*rate, Some(2.0 / 3.0));
    }
}
