// use std::sync::Arc;
use dashmap::DashMap;
use parking_lot::RwLock;

use crate::key::{Interner, KeyId};
use crate::metrics::{MetricHandle, MetricKind, MetricSeriesSummary, MetricStorage, MetricValue};
use crate::tags::TagSet;
use std::sync::atomic::Ordering;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct MetricId(u32);

#[derive(Debug)]
pub struct MetricDef {
    pub name: KeyId,
    pub kind: MetricKind,
}

#[derive(Debug, Default)]
pub struct Registry {
    interner: Interner,
    defs: RwLock<Vec<MetricDef>>,
    storage: DashMap<MetricId, DashMap<TagSet, MetricStorage>>,
}

impl Registry {
    #[must_use]
    pub fn lookup_metric(&self, name: &str) -> Option<(MetricId, MetricKind)> {
        let name_id = self.interner.get_or_intern(name);

        let defs = self.defs.read();
        defs.iter()
            .enumerate()
            .find(|(_, d)| d.name == name_id)
            .map(|(idx, d)| (MetricId(idx as u32), d.kind))
    }

    pub fn register(&self, name: &str, kind: MetricKind) -> MetricId {
        let name_id = self.interner.get_or_intern(name);

        let mut defs = self.defs.write();
        if let Some((idx, _)) = defs.iter().enumerate().find(|(_, d)| d.name == name_id) {
            return MetricId(idx as u32);
        }

        let id = MetricId(defs.len() as u32);
        defs.push(MetricDef {
            name: name_id,
            kind,
        });
        self.storage.insert(id, DashMap::new());
        id
    }

    pub fn resolve_key(&self, key: &str) -> KeyId {
        self.interner.get_or_intern(key)
    }

    pub fn resolve_key_id(&self, id: KeyId) -> Option<std::sync::Arc<str>> {
        self.interner.resolve(id)
    }

    pub fn query(&self, metric: MetricId) -> crate::agg::Query<'_> {
        crate::agg::Query::new(self, metric)
    }

    pub fn resolve_tags(&self, tags: &[(&str, &str)]) -> TagSet {
        let mut resolved: Vec<(KeyId, KeyId)> = tags
            .iter()
            .map(|(k, v)| (self.resolve_key(k), self.resolve_key(v)))
            .collect();
        resolved.sort_unstable();
        TagSet::from_sorted_iter(resolved)
    }

    pub fn get_handle(&self, metric: MetricId, tags: TagSet) -> Option<MetricHandle> {
        let series_map = self.storage.get(&metric)?;

        if let Some(storage) = series_map.get(&tags) {
            return Some(self.storage_to_handle(storage.value()));
        }

        let kind = {
            let defs = self.defs.read();
            defs.get(metric.0 as usize)?.kind
        };

        let new_storage = MetricStorage::new(kind);
        let handle = self.storage_to_handle(&new_storage);
        series_map.insert(tags, new_storage);

        Some(handle)
    }

    fn storage_to_handle(&self, s: &MetricStorage) -> MetricHandle {
        match s {
            MetricStorage::Counter(a) => MetricHandle::Counter(a.clone()),
            MetricStorage::Gauge(a) => MetricHandle::Gauge(a.clone()),
            MetricStorage::Rate(a) => MetricHandle::Rate(a.clone()),
            MetricStorage::Histogram(a) => MetricHandle::Histogram(a.clone()),
        }
    }

    pub fn visit_series<F>(&self, metric: MetricId, mut visit: F)
    where
        F: FnMut(&TagSet, &MetricStorage),
    {
        let Some(series_map) = self.storage.get(&metric) else {
            return;
        };

        for series in series_map.iter() {
            visit(series.key(), series.value());
        }
    }

    pub fn fold_counter_sum<P>(&self, metric: MetricId, mut predicate: P) -> u64
    where
        P: FnMut(&TagSet) -> bool,
    {
        let mut total = 0u64;
        self.visit_series(metric, |tags, storage| {
            if !predicate(tags) {
                return;
            }
            if let MetricStorage::Counter(c) = storage {
                total = total.saturating_add(c.load(Ordering::Relaxed));
            }
        });
        total
    }

    pub fn fold_histogram_summary<P>(
        &self,
        metric: MetricId,
        mut predicate: P,
    ) -> Option<crate::metrics::HistogramSummary>
    where
        P: FnMut(&TagSet) -> bool,
    {
        let mut acc = crate::metrics::new_default_histogram();
        let mut any = false;

        self.visit_series(metric, |tags, storage| {
            if !predicate(tags) {
                return;
            }
            let MetricStorage::Histogram(h) = storage else {
                return;
            };

            any = true;
            let h = h.lock();
            let _ = acc.add(&*h);
        });

        any.then(|| crate::metrics::summarize_histogram(&acc))
    }

    pub fn fold_rate_sum<P>(&self, metric: MetricId, mut predicate: P) -> (u64, u64, Option<f64>)
    where
        P: FnMut(&TagSet) -> bool,
    {
        let mut total = 0u64;
        let mut hits = 0u64;

        self.visit_series(metric, |tags, storage| {
            if !predicate(tags) {
                return;
            }

            let MetricStorage::Rate(r) = storage else {
                return;
            };

            total = total.saturating_add(r.total.load(Ordering::Relaxed));
            hits = hits.saturating_add(r.hits.load(Ordering::Relaxed));
        });

        let rate = (total > 0).then(|| hits as f64 / total as f64);
        (total, hits, rate)
    }

    pub fn summarize(&self) -> Vec<MetricSeriesSummary> {
        let mut out = Vec::new();
        let defs = self.defs.read();

        for entry in self.storage.iter() {
            let metric_id = entry.key();
            let series_map = entry.value();

            let def = match defs.get(metric_id.0 as usize) {
                Some(d) => d,
                None => continue,
            };

            let name_str = self
                .interner
                .resolve(def.name)
                .map(|s| s.to_string())
                .unwrap_or_default();

            for series in series_map.iter() {
                let tags = series.key();
                let storage = series.value();

                let tag_vec: Vec<(String, String)> = tags
                    .tags
                    .iter()
                    .map(|(k, v)| {
                        (
                            self.interner
                                .resolve(*k)
                                .map(|s| s.to_string())
                                .unwrap_or_default(),
                            self.interner
                                .resolve(*v)
                                .map(|s| s.to_string())
                                .unwrap_or_default(),
                        )
                    })
                    .collect();

                let values = match storage {
                    MetricStorage::Counter(a) => MetricValue::Counter(a.load(Ordering::Relaxed)),
                    MetricStorage::Gauge(a) => MetricValue::Gauge(a.load(Ordering::Relaxed)),
                    MetricStorage::Rate(r) => {
                        let total = r.total.load(Ordering::Relaxed);
                        let hits = r.hits.load(Ordering::Relaxed);
                        let rate = if total > 0 {
                            Some(hits as f64 / total as f64)
                        } else {
                            None
                        };
                        MetricValue::Rate { total, hits, rate }
                    }
                    MetricStorage::Histogram(h) => {
                        let h = h.lock();
                        MetricValue::Histogram(crate::metrics::summarize_histogram(&h))
                    }
                };

                out.push(MetricSeriesSummary {
                    name: name_str.clone(),
                    kind: def.kind,
                    tags: tag_vec,
                    values,
                });
            }
        }

        out.sort_by(|a, b| a.name.cmp(&b.name));
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::metrics::MetricKind;
    use std::sync::atomic::Ordering;

    #[test]
    fn fold_counter_sum_filters_by_tags() {
        let reg = Registry::default();
        let m = reg.register("requests_total", MetricKind::Counter);

        let scenario_key = reg.resolve_key("scenario");
        let a = reg.resolve_key("A");
        let b = reg.resolve_key("B");

        let tags_a = TagSet::from_sorted_iter([(scenario_key, a)]);
        let tags_b = TagSet::from_sorted_iter([(scenario_key, b)]);

        if let Some(MetricHandle::Counter(c)) = reg.get_handle(m, tags_a) {
            c.fetch_add(10, Ordering::Relaxed);
        }
        if let Some(MetricHandle::Counter(c)) = reg.get_handle(m, tags_b) {
            c.fetch_add(3, Ordering::Relaxed);
        }

        let sum_a = reg.fold_counter_sum(m, |tags| tags.get(scenario_key) == Some(a));
        let sum_all = reg.fold_counter_sum(m, |_tags| true);

        assert_eq!(sum_a, 10);
        assert_eq!(sum_all, 13);
    }

    #[test]
    fn fold_histogram_summary_merges_series() {
        let reg = Registry::default();
        let m = reg.register("request_latency", MetricKind::Histogram);

        let scenario_key = reg.resolve_key("scenario");
        let a = reg.resolve_key("A");

        let tags1 = TagSet::from_sorted_iter([
            (scenario_key, a),
            (reg.resolve_key("group"), reg.resolve_key("g1")),
        ]);
        let tags2 = TagSet::from_sorted_iter([
            (scenario_key, a),
            (reg.resolve_key("group"), reg.resolve_key("g2")),
        ]);

        if let Some(MetricHandle::Histogram(h)) = reg.get_handle(m, tags1) {
            let mut h = h.lock();
            let _ = h.record(10);
            let _ = h.record(20);
        }
        if let Some(MetricHandle::Histogram(h)) = reg.get_handle(m, tags2) {
            let mut h = h.lock();
            let _ = h.record(30);
        }

        let summary = reg.fold_histogram_summary(m, |tags| tags.get(scenario_key) == Some(a));

        let summary = match summary {
            Some(s) => s,
            None => panic!("expected histogram summary"),
        };

        assert_eq!(summary.count, 3);
        assert_eq!(summary.max, Some(30.0));
        assert_eq!(summary.min, Some(10.0));
    }

    #[test]
    fn lookup_metric_returns_id_and_kind() {
        let reg = Registry::default();
        let _ = reg.register("foo", MetricKind::Counter);

        let (id, kind) = match reg.lookup_metric("foo") {
            Some(v) => v,
            None => panic!("expected metric"),
        };
        assert_eq!(kind, MetricKind::Counter);

        // Ensure the returned id is usable.
        let tags = TagSet::from_sorted_iter([]);
        let Some(MetricHandle::Counter(c)) = reg.get_handle(id, tags) else {
            panic!("expected counter handle");
        };
        c.fetch_add(1, Ordering::Relaxed);
        assert_eq!(reg.fold_counter_sum(id, |_| true), 1);
    }

    #[test]
    fn fold_rate_sum_aggregates_series() {
        let reg = Registry::default();
        let m = reg.register("http_req_failed", MetricKind::Rate);

        let scenario_key = reg.resolve_key("scenario");
        let a = reg.resolve_key("A");
        let b = reg.resolve_key("B");

        let tags_a = TagSet::from_sorted_iter([(scenario_key, a)]);
        let tags_b = TagSet::from_sorted_iter([(scenario_key, b)]);

        if let Some(MetricHandle::Rate(r)) = reg.get_handle(m, tags_a) {
            r.total.fetch_add(10, Ordering::Relaxed);
            r.hits.fetch_add(1, Ordering::Relaxed);
        }
        if let Some(MetricHandle::Rate(r)) = reg.get_handle(m, tags_b) {
            r.total.fetch_add(5, Ordering::Relaxed);
            r.hits.fetch_add(0, Ordering::Relaxed);
        }

        let (total, hits, rate) = reg.fold_rate_sum(m, |_| true);
        assert_eq!(total, 15);
        assert_eq!(hits, 1);
        let Some(rate) = rate else {
            panic!("expected Some(rate)");
        };
        assert!((rate - (1.0 / 15.0)).abs() < 1e-12);
    }
}
