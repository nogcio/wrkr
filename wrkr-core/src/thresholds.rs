#[derive(Debug, Clone)]
pub struct ThresholdSet {
    pub metric: String,
    /// Optional tag selector for this threshold set.
    ///
    /// When non-empty, the threshold is evaluated only against series whose tags match the
    /// selector (order-insensitive).
    pub tags: Vec<(String, String)>,
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
    pub tags: Vec<(String, String)>,
    pub expression: String,
    pub observed: Option<f64>,
}

pub fn parse_threshold_metric_key(raw: &str) -> Result<(String, Vec<(String, String)>), String> {
    let raw = raw.trim();
    if raw.is_empty() {
        return Err("empty metric key".to_string());
    }

    let Some((name_raw, selector_with_brace)) = raw.split_once('{') else {
        return Ok((raw.to_string(), Vec::new()));
    };

    let name = name_raw.trim();
    if name.is_empty() {
        return Err(format!("invalid metric key (missing metric name): {raw}"));
    }

    let selector = selector_with_brace
        .strip_suffix('}')
        .ok_or_else(|| format!("invalid metric key (missing `}}`): {raw}"))?;

    // v1: simple selector values (no escaping/quoting).
    // Whitespace is ignored around tokens, but not allowed inside keys/values.
    let mut tags: Vec<(String, String)> = Vec::new();

    for part in selector.split(',') {
        let part = part.trim();
        if part.is_empty() {
            continue;
        }

        let (k_raw, v_raw) = part
            .split_once('=')
            .ok_or_else(|| format!("invalid selector pair (expected k=v): {raw}"))?;
        let k = k_raw.trim();
        let v = v_raw.trim();
        if k.is_empty() || v.is_empty() {
            return Err(format!("invalid selector pair (empty key/value): {raw}"));
        }

        let is_simple = |s: &str| {
            !s.chars()
                .any(|c| c.is_whitespace() || matches!(c, '{' | '}' | ',' | '='))
        };
        if !is_simple(k) || !is_simple(v) {
            return Err(format!(
                "invalid selector (unsupported characters in key/value): {raw}"
            ));
        }

        if tags.iter().any(|(ek, _)| ek == k) {
            return Err(format!("invalid selector (duplicate tag key `{k}`): {raw}"));
        }

        tags.push((k.to_string(), v.to_string()));
    }

    if tags.is_empty() {
        return Err(format!("invalid metric key (empty selector): {raw}"));
    }

    tags.sort_unstable_by(|a, b| a.0.cmp(&b.0).then_with(|| a.1.cmp(&b.1)));
    Ok((name.to_string(), tags))
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

#[cfg(test)]
mod tests {
    use super::*;

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
    fn parse_threshold_metric_key_without_selector() {
        let (name, tags) =
            parse_threshold_metric_key("http_req_duration").unwrap_or_else(|e| panic!("{e}"));
        assert_eq!(name, "http_req_duration");
        assert!(tags.is_empty());
    }

    #[test]
    fn parse_threshold_metric_key_with_selector_trims_and_sorts() {
        let (name, tags) =
            parse_threshold_metric_key("http_req_duration{ group = login , method=GET }")
                .unwrap_or_else(|e| panic!("{e}"));
        assert_eq!(name, "http_req_duration");
        assert_eq!(
            tags,
            vec![
                ("group".to_string(), "login".to_string()),
                ("method".to_string(), "GET".to_string())
            ]
        );
    }
}
