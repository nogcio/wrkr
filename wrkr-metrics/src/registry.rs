// use std::sync::Arc;
use dashmap::DashMap;
use parking_lot::RwLock;

use crate::key::{Interner, KeyId};
use crate::metrics::{
    HistogramSummary, MetricHandle, MetricKind, MetricSeriesSummary, MetricStorage, MetricValue,
};
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
                        let count = h.len();
                        let map_val = |v| v as f64;

                        MetricValue::Histogram(HistogramSummary {
                            p50: if count > 0 {
                                Some(map_val(h.value_at_quantile(0.50)))
                            } else {
                                None
                            },
                            p90: if count > 0 {
                                Some(map_val(h.value_at_quantile(0.90)))
                            } else {
                                None
                            },
                            p95: if count > 0 {
                                Some(map_val(h.value_at_quantile(0.95)))
                            } else {
                                None
                            },
                            p99: if count > 0 {
                                Some(map_val(h.value_at_quantile(0.99)))
                            } else {
                                None
                            },
                            min: if count > 0 {
                                Some(map_val(h.min()))
                            } else {
                                None
                            },
                            max: if count > 0 {
                                Some(map_val(h.max()))
                            } else {
                                None
                            },
                            mean: if count > 0 { Some(h.mean()) } else { None },
                            count,
                        })
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
