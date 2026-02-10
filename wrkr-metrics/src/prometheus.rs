use std::collections::{HashMap, HashSet};
use std::fmt::Write as _;

use crate::{MetricValue, Registry};

pub mod pushgateway;

/// Encode the full metric `Registry` into Prometheus text exposition format.
///
/// Notes:
/// - This is intended to be Prometheus-native and PromQL-friendly.
/// - `MetricKind::Rate` is exported as:
///   - `<name>_total` (counter)
///   - `<name>_hits_total` (counter)
///   - `<name>_rate` (gauge, optional)
/// - `MetricKind::Histogram` is exported as a Prometheus `histogram` named `<name>`
///   using `_bucket{le="..."}`, `_sum`, and `_count`.
#[must_use]
pub fn registry_to_text(registry: &Registry) -> String {
    fn esc_label_value(v: &str) -> String {
        let mut s = String::with_capacity(v.len());
        for ch in v.chars() {
            match ch {
                '\\' => s.push_str("\\\\"),
                '"' => s.push_str("\\\""),
                '\n' => s.push_str("\\n"),
                _ => s.push(ch),
            }
        }
        s
    }

    fn sanitize_prom_label_key(k: &str) -> String {
        let mut out = String::with_capacity(k.len());
        for (idx, ch) in k.chars().enumerate() {
            let ok = match ch {
                'a'..='z' | 'A'..='Z' | '_' => true,
                '0'..='9' => idx != 0,
                _ => false,
            };
            out.push(if ok { ch } else { '_' });
        }
        if out.is_empty() {
            out.push('_');
        }
        if !out
            .chars()
            .next()
            .is_some_and(|c| matches!(c, 'a'..='z' | 'A'..='Z' | '_'))
        {
            out.insert(0, '_');
        }
        out
    }

    fn sanitize_prom_metric_name(name: &str) -> String {
        // Metric name: [a-zA-Z_:][a-zA-Z0-9_:]*
        let mut out = String::with_capacity(name.len());
        for (idx, ch) in name.chars().enumerate() {
            let ok = match ch {
                'a'..='z' | 'A'..='Z' | '_' | ':' => true,
                '0'..='9' => idx != 0,
                _ => false,
            };
            out.push(if ok { ch } else { '_' });
        }
        if out.is_empty() {
            out.push('_');
        }
        if !out
            .chars()
            .next()
            .is_some_and(|c| matches!(c, 'a'..='z' | 'A'..='Z' | '_' | ':'))
        {
            out.insert(0, '_');
        }
        out
    }

    fn prom_metric_base_name(name: &str) -> String {
        let sanitized = sanitize_prom_metric_name(name);
        if sanitized.starts_with("wrkr_") {
            sanitized
        } else {
            format!("wrkr_{sanitized}")
        }
    }

    fn fmt_labels(tags: &[(String, String)], extra: &[(String, String)]) -> String {
        let mut out = String::new();

        // After sanitization, label keys can collide (e.g. `a-b` and `a_b`).
        // Prometheus rejects duplicate label names within a sample; to avoid the
        // whole scrape failing, we make keys unique by suffixing `_N`.
        let mut seen: HashMap<String, usize> = HashMap::new();

        // IMPORTANT: emit `extra` first so reserved label names like `quantile`
        // keep their canonical key even if user tags contain the same key.
        let mut first = true;
        for (k, v) in extra.iter().chain(tags.iter()) {
            let mut k = sanitize_prom_label_key(k);
            let v = esc_label_value(v);

            let n = seen.entry(k.clone()).or_insert(0);
            if *n > 0 {
                k = format!("{k}_{n}");
            }
            *n += 1;

            if !first {
                out.push(',');
            }
            first = false;
            let _ = write!(out, "{k}=\"{v}\"");
        }

        out
    }

    fn ensure_meta(out: &mut String, emitted: &mut HashSet<String>, name: &str, typ: &str) {
        if emitted.insert(name.to_string()) {
            let _ = writeln!(out, "# HELP {name} wrkr internal metric");
            let _ = writeln!(out, "# TYPE {name} {typ}");
        }
    }

    fn emit_scalar<T: std::fmt::Display>(
        out: &mut String,
        emitted: &mut HashSet<String>,
        name: &str,
        typ: &str,
        tags: &[(String, String)],
        extra: &[(String, String)],
        value: T,
    ) {
        ensure_meta(out, emitted, name, typ);
        let labels = fmt_labels(tags, extra);
        if labels.is_empty() {
            let _ = writeln!(out, "{name} {value}");
        } else {
            let _ = writeln!(out, "{name}{{{labels}}} {value}");
        }
    }

    fn emit_histogram_bucket(
        out: &mut String,
        bucket_name: &str,
        tags: &[(String, String)],
        extra: &[(String, String)],
        le: &str,
        count: u64,
    ) {
        let mut bucket_extra: Vec<(String, String)> = Vec::with_capacity(extra.len() + 1);
        bucket_extra.extend_from_slice(extra);
        bucket_extra.push(("le".to_string(), le.to_string()));

        let labels = fmt_labels(tags, &bucket_extra);
        let _ = writeln!(out, "{bucket_name}{{{labels}}} {count}");
    }

    let mut series = registry.summarize();
    series.sort_by(|a, b| a.name.cmp(&b.name).then_with(|| a.tags.cmp(&b.tags)));

    // Detect kind-collisions that would break Prometheus `# TYPE` invariants.
    let mut kind_sets: HashMap<String, HashSet<crate::metrics::MetricKind>> = HashMap::new();
    for s in &series {
        let base = prom_metric_base_name(&s.name);
        kind_sets.entry(base).or_default().insert(s.kind);
    }

    // Detect name-collisions for the exported Prometheus metric names.
    // If multiple original metric names map to the same exported name, we add a
    // disambiguating label `wrkr_metric="<original>"`.
    let mut exported_name_to_originals: HashMap<String, HashSet<String>> = HashMap::new();
    for s in &series {
        let base = prom_metric_base_name(&s.name);
        let base = if kind_sets.get(&base).is_some_and(|ks| ks.len() > 1) {
            format!("{base}__{}", s.kind.to_string().to_lowercase())
        } else {
            base
        };

        let mut names: Vec<String> = Vec::new();
        match s.kind {
            crate::metrics::MetricKind::Counter | crate::metrics::MetricKind::Gauge => {
                names.push(base);
            }
            crate::metrics::MetricKind::Rate => {
                names.push(format!("{base}_total"));
                names.push(format!("{base}_hits_total"));
                names.push(format!("{base}_rate"));
            }
            crate::metrics::MetricKind::Histogram => {
                names.push(base.clone());
                names.push(format!("{base}_bucket"));
                names.push(format!("{base}_count"));
                names.push(format!("{base}_sum"));
            }
        }

        for name in names {
            exported_name_to_originals
                .entry(name)
                .or_default()
                .insert(s.name.clone());
        }
    }

    let mut out = String::new();
    let mut emitted_meta: HashSet<String> = HashSet::new();

    for s in series {
        let base = prom_metric_base_name(&s.name);
        let base = if kind_sets.get(&base).is_some_and(|ks| ks.len() > 1) {
            format!("{base}__{}", s.kind.to_string().to_lowercase())
        } else {
            base
        };

        let extra_for = |metric_name: &str| -> Vec<(String, String)> {
            let needs_metric_label = exported_name_to_originals
                .get(metric_name)
                .is_some_and(|set| set.len() > 1);
            if needs_metric_label {
                vec![("wrkr_metric".to_string(), s.name.clone())]
            } else {
                Vec::new()
            }
        };

        match s.values {
            MetricValue::Counter(v) => {
                let extra = extra_for(&base);
                emit_scalar(
                    &mut out,
                    &mut emitted_meta,
                    &base,
                    "counter",
                    &s.tags,
                    &extra,
                    v,
                );
            }
            MetricValue::Gauge(v) => {
                let extra = extra_for(&base);
                emit_scalar(
                    &mut out,
                    &mut emitted_meta,
                    &base,
                    "gauge",
                    &s.tags,
                    &extra,
                    v,
                );
            }
            MetricValue::Rate { total, hits, rate } => {
                let total_name = format!("{base}_total");
                let hits_total_name = format!("{base}_hits_total");
                let rate_name = format!("{base}_rate");

                let extra_total = extra_for(&total_name);
                emit_scalar(
                    &mut out,
                    &mut emitted_meta,
                    &total_name,
                    "counter",
                    &s.tags,
                    &extra_total,
                    total,
                );

                let extra_hits_total = extra_for(&hits_total_name);
                emit_scalar(
                    &mut out,
                    &mut emitted_meta,
                    &hits_total_name,
                    "counter",
                    &s.tags,
                    &extra_hits_total,
                    hits,
                );

                if let Some(r) = rate {
                    let extra_rate = extra_for(&rate_name);
                    emit_scalar(
                        &mut out,
                        &mut emitted_meta,
                        &rate_name,
                        "gauge",
                        &s.tags,
                        &extra_rate,
                        r,
                    );
                }
            }
            MetricValue::Histogram(_h) => {
                ensure_meta(&mut out, &mut emitted_meta, &base, "histogram");

                let bucket_name = format!("{base}_bucket");
                let sum_name = format!("{base}_sum");
                let count_name = format!("{base}_count");

                let extra_bucket = extra_for(&bucket_name);
                let extra_sum = extra_for(&sum_name);
                let extra_count = extra_for(&count_name);

                let (buckets, sum, count) = match registry.lookup_metric(&s.name) {
                    Some((metric_id, crate::metrics::MetricKind::Histogram)) => {
                        let tags = registry.resolve_tags(
                            &s.tags
                                .iter()
                                .map(|(k, v)| (k.as_str(), v.as_str()))
                                .collect::<Vec<_>>(),
                        );
                        match registry.get_handle(metric_id, tags) {
                            Some(crate::MetricHandle::Histogram(hh)) => {
                                let snapshot = hh.lock().clone();
                                let count = snapshot.len();
                                let sum = if count > 0 {
                                    snapshot.mean() * (count as f64)
                                } else {
                                    0.0
                                };

                                let mut cumulative = 0u64;
                                let mut buckets: Vec<(u64, u64)> = Vec::new();
                                for v in snapshot.iter_recorded() {
                                    cumulative =
                                        cumulative.saturating_add(v.count_since_last_iteration());
                                    buckets.push((v.value_iterated_to(), cumulative));
                                }

                                (buckets, sum, count)
                            }
                            _ => (Vec::new(), 0.0, 0),
                        }
                    }
                    _ => (Vec::new(), 0.0, 0),
                };

                for (le, c) in buckets {
                    emit_histogram_bucket(
                        &mut out,
                        &bucket_name,
                        &s.tags,
                        &extra_bucket,
                        &le.to_string(),
                        c,
                    );
                }
                emit_histogram_bucket(
                    &mut out,
                    &bucket_name,
                    &s.tags,
                    &extra_bucket,
                    "+Inf",
                    count,
                );

                let labels_sum = fmt_labels(&s.tags, &extra_sum);
                let labels_count = fmt_labels(&s.tags, &extra_count);

                if labels_sum.is_empty() {
                    let _ = writeln!(out, "{sum_name} {sum}");
                } else {
                    let _ = writeln!(out, "{sum_name}{{{labels_sum}}} {sum}");
                }

                if labels_count.is_empty() {
                    let _ = writeln!(out, "{count_name} {count}");
                } else {
                    let _ = writeln!(out, "{count_name}{{{labels_count}}} {count}");
                }
            }
        }
    }

    out
}

#[cfg(test)]
mod tests {
    use crate::{MetricKind, Registry};

    use super::registry_to_text;

    #[test]
    fn encodes_counter_with_sanitized_names_and_escaped_labels() {
        let metrics = Registry::default();
        let metric = metrics.register("a-b", MetricKind::Counter);
        let tags = metrics.resolve_tags(&[("x-y", "hello\"\\\nworld")]);
        let h = match metrics.get_handle(metric, tags) {
            Some(h) => h,
            None => panic!("handle"),
        };
        h.increment(3);

        let text = registry_to_text(&metrics);
        assert!(text.contains("wrkr_a_b{"));
        assert!(text.contains("x_y=\"hello\\\"\\\\\\nworld\""));
        assert!(text.contains(" 3\n"));
    }

    #[test]
    fn encodes_rate_as_native_suffix_metrics() {
        let metrics = Registry::default();
        let metric = metrics.register("checks", MetricKind::Rate);
        let tags = metrics.resolve_tags(&[("scenario", "s1")]);

        let h = match metrics.get_handle(metric, tags) {
            Some(h) => h,
            None => panic!("handle"),
        };
        // one "hit" out of one total
        h.add_rate(1, 1);

        let text = registry_to_text(&metrics);
        assert!(text.contains("# TYPE wrkr_checks_total counter\n"));
        assert!(text.contains("wrkr_checks_total{"));
        assert!(text.contains("# TYPE wrkr_checks_hits_total counter\n"));
        assert!(text.contains("wrkr_checks_hits_total{"));
    }

    #[test]
    fn encodes_histogram_as_summary() {
        let metrics = Registry::default();
        let metric = metrics.register("latency", MetricKind::Histogram);
        let tags = metrics.resolve_tags(&[("scenario", "s1")]);

        let h = match metrics.get_handle(metric, tags) {
            Some(h) => h,
            None => panic!("handle"),
        };
        h.observe_histogram(10);
        h.observe_histogram(20);
        h.observe_histogram(30);

        let text = registry_to_text(&metrics);

        assert!(text.contains("# TYPE wrkr_latency histogram\n"));
        assert!(text.contains("wrkr_latency_bucket{le=\"+Inf\""));
        assert!(text.contains("wrkr_latency_count{"));
        assert!(text.contains("wrkr_latency_sum{"));
    }
}
