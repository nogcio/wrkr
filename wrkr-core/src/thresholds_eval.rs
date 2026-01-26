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
        let Some((metric_id, kind)) = metrics.lookup_metric(&set.metric) else {
            // Missing metric => all expressions fail.
            for expr in &set.expressions {
                out.push(ThresholdViolation {
                    metric: set.metric.clone(),
                    expression: expr.clone(),
                    observed: None,
                });
            }
            continue;
        };

        for expr_raw in &set.expressions {
            let expr =
                parse_threshold_expr(expr_raw).map_err(|error| Error::InvalidThresholdExpr {
                    metric: set.metric.clone(),
                    error,
                })?;

            let observed = observed_value(metrics, metric_id, kind, &expr.agg);

            let passed = observed.is_some_and(|v| compare(v, expr.op, expr.value));
            if !passed {
                out.push(ThresholdViolation {
                    metric: set.metric.clone(),
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
) -> Option<f64> {
    match agg {
        ThresholdAgg::Count => match kind {
            MetricKind::Counter => Some(metrics.fold_counter_sum(metric_id, |_| true) as f64),
            MetricKind::Rate => {
                let (total, _hits, _rate) = metrics.fold_rate_sum(metric_id, |_| true);
                Some(total as f64)
            }
            MetricKind::Histogram => metrics
                .fold_histogram_summary(metric_id, |_| true)
                .map(|h| h.count as f64),
            MetricKind::Gauge => None,
        },

        ThresholdAgg::Rate => match kind {
            MetricKind::Rate => {
                let (_total, _hits, rate) = metrics.fold_rate_sum(metric_id, |_| true);
                rate
            }
            _ => None,
        },

        ThresholdAgg::Avg => match kind {
            MetricKind::Histogram => metrics
                .fold_histogram_summary(metric_id, |_| true)
                .and_then(|h| h.mean),
            _ => None,
        },

        ThresholdAgg::Min => match kind {
            MetricKind::Histogram => metrics
                .fold_histogram_summary(metric_id, |_| true)
                .and_then(|h| h.min),
            _ => None,
        },

        ThresholdAgg::Max => match kind {
            MetricKind::Histogram => metrics
                .fold_histogram_summary(metric_id, |_| true)
                .and_then(|h| h.max),
            _ => None,
        },

        ThresholdAgg::P(p) => match kind {
            MetricKind::Histogram => metrics
                .fold_histogram_summary(metric_id, |_| true)
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
            expressions: vec!["rate<0.2".to_string()],
        }];

        let v = match evaluate_thresholds(&metrics, &sets) {
            Ok(v) => v,
            Err(e) => panic!("unexpected error: {e}"),
        };
        assert!(v.is_empty());
    }
}
