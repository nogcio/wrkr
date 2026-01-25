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
}
