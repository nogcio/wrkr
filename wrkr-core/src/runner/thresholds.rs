use super::metrics::{MetricSeriesSummary, MetricValues};

#[derive(Debug, Clone)]
pub struct ThresholdSet {
    pub metric: String,
    pub expressions: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ThresholdOp {
    Lt,
    Lte,
    Gt,
    Gte,
    Eq,
}

#[derive(Debug, Clone)]
pub enum ThresholdAgg {
    Avg,
    Min,
    Max,
    Count,
    Rate,
    P(u32),
}

#[derive(Debug, Clone)]
pub struct ThresholdExpr {
    pub agg: ThresholdAgg,
    pub op: ThresholdOp,
    pub value: f64,
}

#[derive(Debug, Clone)]
pub struct ThresholdViolation {
    pub metric: String,
    pub expression: String,
    pub observed: Option<f64>,
}

pub fn parse_threshold_expr(raw: &str) -> Result<ThresholdExpr, String> {
    let s: String = raw.chars().filter(|c| !c.is_whitespace()).collect();
    if s.is_empty() {
        return Err("empty threshold".to_string());
    }

    // Find operator
    let ops = [
        ("<=", ThresholdOp::Lte),
        (">=", ThresholdOp::Gte),
        ("==", ThresholdOp::Eq),
        ("<", ThresholdOp::Lt),
        (">", ThresholdOp::Gt),
    ];
    let (op_pos, op_len, op) = ops
        .iter()
        .find_map(|(tok, op)| s.find(tok).map(|pos| (pos, tok.len(), *op)))
        .ok_or_else(|| format!("invalid threshold (missing operator): {raw}"))?;

    let (left, right_with_op) = s.split_at(op_pos);
    let right = &right_with_op[op_len..];
    if left.is_empty() || right.is_empty() {
        return Err(format!("invalid threshold: {raw}"));
    }

    let agg = if left.eq_ignore_ascii_case("avg") {
        ThresholdAgg::Avg
    } else if left.eq_ignore_ascii_case("min") {
        ThresholdAgg::Min
    } else if left.eq_ignore_ascii_case("max") {
        ThresholdAgg::Max
    } else if left.eq_ignore_ascii_case("count") {
        ThresholdAgg::Count
    } else if left.eq_ignore_ascii_case("rate") {
        ThresholdAgg::Rate
    } else if let Some(inner) = left.strip_prefix("p(").and_then(|v| v.strip_suffix(')')) {
        let p: u32 = inner
            .parse()
            .map_err(|_| format!("invalid percentile in threshold: {raw}"))?;
        if !(1..=100).contains(&p) {
            return Err(format!("percentile out of range in threshold: {raw}"));
        }
        ThresholdAgg::P(p)
    } else {
        return Err(format!("unknown aggregation `{left}` in threshold: {raw}"));
    };

    let value: f64 = right
        .parse()
        .map_err(|_| format!("invalid numeric value in threshold: {raw}"))?;

    Ok(ThresholdExpr { agg, op, value })
}

pub fn evaluate_thresholds(
    thresholds: &[ThresholdSet],
    metrics: &[MetricSeriesSummary],
) -> Result<Vec<ThresholdViolation>, String> {
    let mut out = Vec::new();

    for set in thresholds {
        let metric_name = set.metric.as_str();
        let series = metrics
            .iter()
            .find(|m| m.name == metric_name && m.tags.is_empty());

        for expr_raw in &set.expressions {
            let expr = parse_threshold_expr(expr_raw)?;
            let observed = series.and_then(|s| observed_value(s, &expr.agg));
            let passed = observed
                .map(|v| compare(v, expr.op, expr.value))
                .unwrap_or(false);
            if !passed {
                out.push(ThresholdViolation {
                    metric: metric_name.to_string(),
                    expression: expr_raw.to_string(),
                    observed,
                });
            }
        }
    }

    Ok(out)
}

fn compare(left: f64, op: ThresholdOp, right: f64) -> bool {
    match op {
        ThresholdOp::Lt => left < right,
        ThresholdOp::Lte => left <= right,
        ThresholdOp::Gt => left > right,
        ThresholdOp::Gte => left >= right,
        ThresholdOp::Eq => left == right,
    }
}

fn observed_value(series: &MetricSeriesSummary, agg: &ThresholdAgg) -> Option<f64> {
    match (&series.values, agg) {
        (MetricValues::Trend { avg, .. }, ThresholdAgg::Avg) => *avg,
        (MetricValues::Trend { min, .. }, ThresholdAgg::Min) => *min,
        (MetricValues::Trend { max, .. }, ThresholdAgg::Max) => *max,
        (MetricValues::Trend { count, .. }, ThresholdAgg::Count) => Some(*count as f64),
        (
            MetricValues::Trend {
                p50, p90, p95, p99, ..
            },
            ThresholdAgg::P(p),
        ) => match *p {
            50 => *p50,
            90 => *p90,
            95 => *p95,
            99 => *p99,
            // For other percentiles, we currently only support common ones.
            _ => None,
        },

        (MetricValues::Counter { value }, ThresholdAgg::Count) => Some(*value),
        (MetricValues::Counter { value }, ThresholdAgg::Avg) => Some(*value),
        (MetricValues::Gauge { value }, ThresholdAgg::Avg) => Some(*value),
        (MetricValues::Gauge { value }, ThresholdAgg::Min) => Some(*value),
        (MetricValues::Gauge { value }, ThresholdAgg::Max) => Some(*value),

        (MetricValues::Rate { rate, .. }, ThresholdAgg::Rate) => *rate,
        (MetricValues::Rate { total, .. }, ThresholdAgg::Count) => Some(*total as f64),

        // Non-sensical combinations.
        (_, _) => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::runner::metrics::{MetricKind, MetricSeriesSummary, MetricValues};

    #[test]
    fn parse_threshold_expr_trims_whitespace() {
        let expr = parse_threshold_expr("  avg  <=  123  ").unwrap_or_else(|e| panic!("{e}"));
        assert!(matches!(expr.agg, ThresholdAgg::Avg));
        assert!(matches!(expr.op, ThresholdOp::Lte));
        assert_eq!(expr.value, 123.0);
    }

    #[test]
    fn parse_threshold_expr_rejects_out_of_range_percentiles() {
        let err = match parse_threshold_expr("p(101)<1") {
            Ok(_) => panic!("expected error"),
            Err(e) => e,
        };
        assert!(err.contains("out of range"));
    }

    #[test]
    fn evaluate_thresholds_flags_missing_series() {
        let thresholds = vec![ThresholdSet {
            metric: "does_not_exist".to_string(),
            expressions: vec!["avg>0".to_string()],
        }];

        let metrics: Vec<MetricSeriesSummary> = Vec::new();
        let violations =
            evaluate_thresholds(&thresholds, &metrics).unwrap_or_else(|e| panic!("{e}"));
        assert_eq!(violations.len(), 1);
        assert_eq!(violations[0].observed, None);
    }

    #[test]
    fn evaluate_thresholds_uses_base_series_only() {
        let thresholds = vec![ThresholdSet {
            metric: "m".to_string(),
            expressions: vec!["count==1".to_string()],
        }];

        let metrics = vec![
            MetricSeriesSummary {
                name: "m".to_string(),
                kind: MetricKind::Counter,
                tags: vec![("t".to_string(), "x".to_string())],
                values: MetricValues::Counter { value: 1.0 },
            },
            MetricSeriesSummary {
                name: "m".to_string(),
                kind: MetricKind::Counter,
                tags: Vec::new(),
                values: MetricValues::Counter { value: 0.0 },
            },
        ];

        let violations =
            evaluate_thresholds(&thresholds, &metrics).unwrap_or_else(|e| panic!("{e}"));
        assert_eq!(violations.len(), 1);
        assert_eq!(violations[0].observed, Some(0.0));
    }
}
