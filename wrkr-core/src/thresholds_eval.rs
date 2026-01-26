use crate::{ThresholdAgg, ThresholdOp, ThresholdSet, ThresholdViolation, parse_threshold_expr};
use wrkr_metrics::{MetricKind, Registry};

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("invalid threshold expression for metric `{metric}`: {error}")]
    InvalidThresholdExpr { metric: String, error: String },
}

pub fn evaluate_thresholds(
    metrics: &Registry,
    sets: &[ThresholdSet],
) -> Result<Vec<ThresholdViolation>> {
    let mut out: Vec<ThresholdViolation> = Vec::new();

    for set in sets {
        let selector = TagSelector::new(metrics, &set.tags);

        let Some((metric_id, kind)) = metrics.lookup_metric(&set.metric) else {
            // Missing metric => all expressions fail.
            for expr in &set.expressions {
                out.push(ThresholdViolation {
                    metric: set.metric.clone(),
                    tags: set.tags.clone(),
                    expression: expr.clone(),
                    observed: None,
                });
            }
            continue;
        };

        let any_series = selector.any_series(metrics, metric_id);

        for expr_raw in &set.expressions {
            let expr =
                parse_threshold_expr(expr_raw).map_err(|error| Error::InvalidThresholdExpr {
                    metric: set.metric.clone(),
                    error,
                })?;

            let observed = any_series
                .then(|| observed_value(metrics, metric_id, kind, &expr.agg, &selector))
                .flatten();

            let passed = observed.is_some_and(|v| compare(v, expr.op, expr.value));
            if !passed {
                out.push(ThresholdViolation {
                    metric: set.metric.clone(),
                    tags: set.tags.clone(),
                    expression: expr_raw.clone(),
                    observed,
                });
            }
        }
    }

    Ok(out)
}

fn observed_value(
    metrics: &Registry,
    metric_id: wrkr_metrics::MetricId,
    kind: MetricKind,
    agg: &ThresholdAgg,
    selector: &TagSelector,
) -> Option<f64> {
    match agg {
        ThresholdAgg::Count => match kind {
            MetricKind::Counter => {
                Some(metrics.fold_counter_sum(metric_id, |tags| selector.matches(tags)) as f64)
            }
            MetricKind::Rate => {
                let (total, _hits, _rate) =
                    metrics.fold_rate_sum(metric_id, |tags| selector.matches(tags));
                Some(total as f64)
            }
            MetricKind::Histogram => metrics
                .fold_histogram_summary(metric_id, |tags| selector.matches(tags))
                .map(|h| h.count as f64),
            MetricKind::Gauge => None,
        },

        ThresholdAgg::Rate => match kind {
            MetricKind::Rate => {
                let (_total, _hits, rate) =
                    metrics.fold_rate_sum(metric_id, |tags| selector.matches(tags));
                rate
            }
            _ => None,
        },

        ThresholdAgg::Avg => match kind {
            MetricKind::Histogram => metrics
                .fold_histogram_summary(metric_id, |tags| selector.matches(tags))
                .and_then(|h| h.mean),
            _ => None,
        },

        ThresholdAgg::Min => match kind {
            MetricKind::Histogram => metrics
                .fold_histogram_summary(metric_id, |tags| selector.matches(tags))
                .and_then(|h| h.min),
            _ => None,
        },

        ThresholdAgg::Max => match kind {
            MetricKind::Histogram => metrics
                .fold_histogram_summary(metric_id, |tags| selector.matches(tags))
                .and_then(|h| h.max),
            _ => None,
        },

        ThresholdAgg::P(p) => match kind {
            MetricKind::Histogram => metrics
                .fold_histogram_summary(metric_id, |tags| selector.matches(tags))
                .and_then(|h| match *p {
                    50 => h.p50,
                    75 => h.p75,
                    90 => h.p90,
                    95 => h.p95,
                    99 => h.p99,
                    _ => None,
                }),
            _ => None,
        },
    }
}

#[derive(Debug, Clone)]
struct TagSelector {
    mode: TagSelectorMode,
}

#[derive(Debug, Clone)]
enum TagSelectorMode {
    All,
    Selector {
        keys: Vec<wrkr_metrics::KeyId>,
        tags: wrkr_metrics::TagSet,
    },
}

impl TagSelector {
    fn new(metrics: &Registry, selector_tags: &[(String, String)]) -> Self {
        if selector_tags.is_empty() {
            return Self {
                mode: TagSelectorMode::All,
            };
        }

        let key_ids = selector_tags
            .iter()
            .map(|(k, _v)| metrics.resolve_key(k))
            .collect::<Vec<_>>();

        let tag_refs: Vec<(&str, &str)> = selector_tags
            .iter()
            .map(|(k, v)| (k.as_str(), v.as_str()))
            .collect();
        let tags = metrics.resolve_tags(&tag_refs);

        Self {
            mode: TagSelectorMode::Selector {
                keys: key_ids,
                tags,
            },
        }
    }

    fn matches(&self, series_tags: &wrkr_metrics::TagSet) -> bool {
        match &self.mode {
            TagSelectorMode::All => true,
            TagSelectorMode::Selector { keys, tags } => series_tags.project(keys) == tags.clone(),
        }
    }

    fn any_series(&self, metrics: &Registry, metric_id: wrkr_metrics::MetricId) -> bool {
        let mut any = false;
        metrics.visit_series(metric_id, |tags, _storage| {
            if self.matches(tags) {
                any = true;
            }
        });
        any
    }
}

fn compare(observed: f64, op: ThresholdOp, expected: f64) -> bool {
    match op {
        ThresholdOp::Lt => observed < expected,
        ThresholdOp::Lte => observed <= expected,
        ThresholdOp::Gt => observed > expected,
        ThresholdOp::Gte => observed >= expected,
        ThresholdOp::Eq => observed == expected,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::Ordering;
    use wrkr_metrics::{MetricHandle, MetricKind, TagSet};

    #[test]
    fn missing_metric_fails_threshold() {
        let metrics = Registry::default();
        let sets = vec![ThresholdSet {
            metric: "nope".to_string(),
            tags: Vec::new(),
            expressions: vec!["count>0".to_string()],
        }];

        let v = match evaluate_thresholds(&metrics, &sets) {
            Ok(v) => v,
            Err(e) => panic!("unexpected error: {e}"),
        };
        assert_eq!(v.len(), 1);
        assert_eq!(v[0].metric, "nope");
        assert!(v[0].observed.is_none());
    }

    #[test]
    fn counter_count_uses_sum() {
        let metrics = Registry::default();
        let id = metrics.register("my_counter", MetricKind::Counter);
        let tags = TagSet::from_sorted_iter([]);
        if let Some(MetricHandle::Counter(c)) = metrics.get_handle(id, tags) {
            c.fetch_add(2, Ordering::Relaxed);
        }

        let sets = vec![ThresholdSet {
            metric: "my_counter".to_string(),
            tags: Vec::new(),
            expressions: vec!["count==2".to_string()],
        }];

        let v = match evaluate_thresholds(&metrics, &sets) {
            Ok(v) => v,
            Err(e) => panic!("unexpected error: {e}"),
        };
        assert!(v.is_empty());
    }

    #[test]
    fn rate_rate_uses_hits_over_total() {
        let metrics = Registry::default();
        let id = metrics.register("http_req_failed", MetricKind::Rate);
        let tags = TagSet::from_sorted_iter([]);
        if let Some(MetricHandle::Rate(r)) = metrics.get_handle(id, tags) {
            r.total.fetch_add(10, Ordering::Relaxed);
            r.hits.fetch_add(1, Ordering::Relaxed);
        }

        let sets = vec![ThresholdSet {
            metric: "http_req_failed".to_string(),
            tags: Vec::new(),
            expressions: vec!["rate<0.2".to_string()],
        }];

        let v = match evaluate_thresholds(&metrics, &sets) {
            Ok(v) => v,
            Err(e) => panic!("unexpected error: {e}"),
        };
        assert!(v.is_empty());
    }

    #[test]
    fn tag_scoped_threshold_matches_series_by_projected_keys() {
        let metrics = Registry::default();
        let id = metrics.register("my_counter", MetricKind::Counter);

        let tags_login = metrics.resolve_tags(&[("scenario", "Default"), ("group", "login")]);
        if let Some(MetricHandle::Counter(c)) = metrics.get_handle(id, tags_login) {
            c.fetch_add(2, Ordering::Relaxed);
        }

        let tags_other = metrics.resolve_tags(&[("scenario", "Default"), ("group", "other")]);
        if let Some(MetricHandle::Counter(c)) = metrics.get_handle(id, tags_other) {
            c.fetch_add(999, Ordering::Relaxed);
        }

        let sets = vec![ThresholdSet {
            metric: "my_counter".to_string(),
            tags: vec![("group".to_string(), "login".to_string())],
            expressions: vec!["count==2".to_string()],
        }];

        let v = evaluate_thresholds(&metrics, &sets).unwrap_or_else(|e| panic!("{e}"));
        assert!(v.is_empty());
    }

    #[test]
    fn missing_tag_scoped_series_fails_with_observed_none() {
        let metrics = Registry::default();
        let _id = metrics.register("my_counter", MetricKind::Counter);

        let sets = vec![ThresholdSet {
            metric: "my_counter".to_string(),
            tags: vec![("group".to_string(), "missing".to_string())],
            expressions: vec!["count>0".to_string()],
        }];

        let v = evaluate_thresholds(&metrics, &sets).unwrap_or_else(|e| panic!("{e}"));
        assert_eq!(v.len(), 1);
        assert_eq!(v[0].metric, "my_counter");
        assert_eq!(
            v[0].tags,
            vec![("group".to_string(), "missing".to_string())]
        );
        assert!(v[0].observed.is_none());
    }
}
