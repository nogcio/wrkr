use std::collections::HashMap;
use std::sync::atomic::Ordering;

use hdrhistogram::Histogram;
use smallvec::SmallVec;

use crate::key::KeyId;
use crate::metrics::{HistogramSummary, MetricStorage, new_default_histogram, summarize_histogram};
use crate::registry::{MetricId, Registry};
use crate::tags::TagSet;

#[derive(Debug, Clone, Copy)]
enum TagFilter {
    Eq(KeyId, KeyId),
    NotEq(KeyId, KeyId),
    Has(KeyId),
    Missing(KeyId),
}

impl TagFilter {
    fn matches(&self, tags: &TagSet) -> bool {
        match *self {
            TagFilter::Eq(k, v) => tags.get(k) == Some(v),
            TagFilter::NotEq(k, v) => tags.get(k) != Some(v),
            TagFilter::Has(k) => tags.get(k).is_some(),
            TagFilter::Missing(k) => tags.get(k).is_none(),
        }
    }
}

#[derive(Debug, Default, Clone, Copy)]
pub struct RunningStats {
    n: u64,
    mean: f64,
    m2: f64,
    max: f64,
}

impl RunningStats {
    pub fn push(&mut self, x: f64) {
        self.n = self.n.saturating_add(1);
        let n_f = self.n as f64;

        let delta = x - self.mean;
        self.mean += delta / n_f;
        let delta2 = x - self.mean;
        self.m2 += delta * delta2;

        if x > self.max {
            self.max = x;
        }
    }

    pub fn mean(&self) -> f64 {
        self.mean
    }

    pub fn stdev(&self) -> f64 {
        if self.n < 2 {
            return 0.0;
        }
        (self.m2 / (self.n as f64 - 1.0)).sqrt()
    }

    pub fn max(&self) -> f64 {
        self.max
    }

    pub fn stdev_pct(&self) -> f64 {
        let mean = self.mean();
        if mean <= 0.0 {
            return 0.0;
        }
        (self.stdev() / mean) * 100.0
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct CounterSnapshot {
    pub total: u64,
}

impl CounterSnapshot {
    pub fn new(total: u64) -> Self {
        Self { total }
    }

    pub fn delta_since(self, prev: Option<Self>) -> u64 {
        match prev {
            Some(prev) => self.total.saturating_sub(prev.total),
            None => self.total,
        }
    }

    pub fn per_sec_since(self, prev: Option<Self>, dt_secs: f64) -> f64 {
        per_sec(self.delta_since(prev), dt_secs)
    }
}

#[inline]
pub fn per_sec(delta: u64, dt_secs: f64) -> f64 {
    // `dt_secs` should always be > 0 in practice, but we defensively clamp to avoid
    // division-by-zero.
    let dt = dt_secs.max(1e-9);
    delta as f64 / dt
}

#[inline]
pub fn per_sec_u64_rounded(delta: u64, dt_secs: f64) -> u64 {
    per_sec(delta, dt_secs).round() as u64
}

#[derive(Debug, Clone)]
pub struct Query<'a> {
    registry: &'a Registry,
    metric: MetricId,
    filters: SmallVec<[TagFilter; 4]>,
    group_keys: SmallVec<[KeyId; 4]>,
}

impl<'a> Query<'a> {
    pub(crate) fn new(registry: &'a Registry, metric: MetricId) -> Self {
        Self {
            registry,
            metric,
            filters: SmallVec::new(),
            group_keys: SmallVec::new(),
        }
    }

    #[must_use]
    pub fn where_eq(mut self, key: KeyId, value: KeyId) -> Self {
        self.filters.push(TagFilter::Eq(key, value));
        self
    }

    #[must_use]
    pub fn where_not_eq(mut self, key: KeyId, value: KeyId) -> Self {
        self.filters.push(TagFilter::NotEq(key, value));
        self
    }

    #[must_use]
    pub fn where_has(mut self, key: KeyId) -> Self {
        self.filters.push(TagFilter::Has(key));
        self
    }

    #[must_use]
    pub fn where_missing(mut self, key: KeyId) -> Self {
        self.filters.push(TagFilter::Missing(key));
        self
    }

    #[must_use]
    pub fn group_by(mut self, keys: impl IntoIterator<Item = KeyId>) -> Self {
        self.group_keys = keys.into_iter().collect();
        self.group_keys.sort_unstable();
        self.group_keys.dedup();
        self
    }

    fn matches(&self, tags: &TagSet) -> bool {
        self.filters.iter().all(|f| f.matches(tags))
    }

    fn group_key(&self, tags: &TagSet) -> TagSet {
        tags.project(&self.group_keys)
    }

    pub fn sum_counter(self) -> HashMap<TagSet, u64> {
        let mut out: HashMap<TagSet, u64> = HashMap::new();

        self.registry.visit_series(self.metric, |tags, storage| {
            if !self.matches(tags) {
                return;
            }
            let MetricStorage::Counter(c) = storage else {
                return;
            };

            let v = c.load(Ordering::Relaxed);
            if v == 0 {
                return;
            }

            let k = self.group_key(tags);
            out.entry(k)
                .and_modify(|cur| *cur = cur.saturating_add(v))
                .or_insert(v);
        });

        out
    }

    pub fn sum_counter_total(self) -> u64 {
        self.sum_counter().values().copied().sum()
    }

    pub fn merge_histogram_summary(self) -> HashMap<TagSet, HistogramSummary> {
        let mut acc: HashMap<TagSet, Histogram<u64>> = HashMap::new();

        self.registry.visit_series(self.metric, |tags, storage| {
            if !self.matches(tags) {
                return;
            }
            let MetricStorage::Histogram(h) = storage else {
                return;
            };

            let k = self.group_key(tags);
            let entry = acc.entry(k).or_insert_with(new_default_histogram);

            let h = h.lock();
            let _ = entry.add(&*h);
        });

        acc.into_iter()
            .map(|(k, h)| (k, summarize_histogram(&h)))
            .collect()
    }

    pub fn merge_histogram_summary_single(self) -> Option<HistogramSummary> {
        let grouped = self.merge_histogram_summary();
        if grouped.is_empty() {
            return None;
        }

        // If caller didn't group_by, key will be empty TagSet.
        if grouped.len() == 1 {
            return grouped.into_iter().next().map(|(_, v)| v);
        }

        // Multiple groups: no single summary.
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::metrics::{MetricHandle, MetricKind};

    #[test]
    fn counter_snapshot_delta_and_rate() {
        let now = CounterSnapshot::new(10);
        assert_eq!(now.delta_since(None), 10);
        assert_eq!(now.delta_since(Some(CounterSnapshot::new(7))), 3);
        assert_eq!(now.delta_since(Some(CounterSnapshot::new(999))), 0);

        let rps = now.per_sec_since(Some(CounterSnapshot::new(7)), 1.0);
        assert!((rps - 3.0).abs() < 1e-9);

        // Defensive clamp: dt=0 should not panic.
        let _ = now.per_sec_since(Some(CounterSnapshot::new(7)), 0.0);
    }

    #[test]
    fn per_sec_u64_rounded_matches_rounding() {
        assert_eq!(per_sec_u64_rounded(3, 2.0), 2);
        assert_eq!(per_sec_u64_rounded(1, 2.0), 1);
        assert_eq!(per_sec_u64_rounded(0, 2.0), 0);
    }

    #[test]
    fn query_sum_counter_groups_and_filters() {
        let reg = Registry::default();
        let metric = reg.register("requests_total", MetricKind::Counter);

        let scenario_k = reg.resolve_key("scenario");
        let protocol_k = reg.resolve_key("protocol");
        let a = reg.resolve_key("A");
        let http = reg.resolve_key("http");
        let grpc = reg.resolve_key("grpc");

        let tags_http = TagSet::from_sorted_iter([(scenario_k, a), (protocol_k, http)]);
        let tags_grpc = TagSet::from_sorted_iter([(scenario_k, a), (protocol_k, grpc)]);

        if let Some(MetricHandle::Counter(c)) = reg.get_handle(metric, tags_http) {
            c.fetch_add(10, Ordering::Relaxed);
        }
        if let Some(MetricHandle::Counter(c)) = reg.get_handle(metric, tags_grpc) {
            c.fetch_add(3, Ordering::Relaxed);
        }

        let grouped = reg
            .query(metric)
            .where_eq(scenario_k, a)
            .group_by([protocol_k])
            .sum_counter();

        assert_eq!(grouped.values().copied().sum::<u64>(), 13);
        assert_eq!(grouped.len(), 2);

        let only_http = reg
            .query(metric)
            .where_eq(scenario_k, a)
            .where_eq(protocol_k, http)
            .sum_counter_total();
        assert_eq!(only_http, 10);
    }

    #[test]
    fn query_merge_histogram_respects_missing_tag_filter() {
        let reg = Registry::default();
        let metric = reg.register("request_latency_ms", MetricKind::Histogram);

        let scenario_k = reg.resolve_key("scenario");
        let protocol_k = reg.resolve_key("protocol");
        let a = reg.resolve_key("A");
        let http = reg.resolve_key("http");

        // overall series: scenario only
        let tags_overall = TagSet::from_sorted_iter([(scenario_k, a)]);
        // protocol series: scenario+protocol
        let tags_http = TagSet::from_sorted_iter([(scenario_k, a), (protocol_k, http)]);

        if let Some(MetricHandle::Histogram(h)) = reg.get_handle(metric, tags_overall) {
            let mut h = h.lock();
            let _ = h.record(10);
            let _ = h.record(20);
        }
        if let Some(MetricHandle::Histogram(h)) = reg.get_handle(metric, tags_http) {
            let mut h = h.lock();
            let _ = h.record(999);
        }

        let summary = reg
            .query(metric)
            .where_eq(scenario_k, a)
            .where_missing(protocol_k)
            .merge_histogram_summary_single()
            .unwrap_or_else(|| panic!("expected summary"));

        assert_eq!(summary.count, 2);
        assert_eq!(summary.max, Some(20.0));
    }
}
