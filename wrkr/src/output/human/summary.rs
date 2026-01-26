use std::collections::{BTreeMap, HashMap};
use std::fmt::Write as _;
use std::time::Duration;

use crate::output::human::format::format_duration_from_micros_opt;

use super::format::*;

pub(crate) fn render(
    summary: &wrkr_core::RunSummary,
    metric_series: Option<&[wrkr_core::MetricSeriesSummary]>,
    run_elapsed: Option<Duration>,
) -> String {
    let mut out = String::new();

    if summary.scenarios.is_empty() {
        out.push_str("summary: no scenarios\n");
        if let Some(series) = metric_series {
            render_checks(series, &mut out);
            render_metrics(series, &mut out);
        }
        return out;
    }

    out.push_str("summary\n");

    let mut totals = Totals::default();

    for s in &summary.scenarios {
        totals.add(s);

        writeln!(&mut out, "scenario: {}", s.scenario).ok();
        writeln!(
            &mut out,
            "  requests: {} (failed {})",
            s.requests_total, s.failed_requests_total
        )
        .ok();
        writeln!(&mut out, "  iterations: {}", s.iterations_total).ok();
        writeln!(
            &mut out,
            "  bytes: recv {} sent {}",
            format_bytes(s.bytes_received_total),
            format_bytes(s.bytes_sent_total)
        )
        .ok();

        if s.checks_failed_total > 0 {
            writeln!(&mut out, "  checks_failed_total: {}", s.checks_failed_total).ok();

            let mut checks: Vec<_> = s.checks_failed.iter().collect();
            checks.sort_by(|(a_name, a_count), (b_name, b_count)| {
                b_count
                    .cmp(a_count)
                    .then_with(|| a_name.as_str().cmp(b_name.as_str()))
            });

            for (name, count) in checks {
                writeln!(&mut out, "    {name}: {count}").ok();
            }
        }

        if let Some(h) = &s.latency {
            writeln!(
                out,
                "  latency = p50={} p90={} p99={} mean={} max={} (n={})",
                format_duration_from_micros_opt(h.p50),
                format_duration_from_micros_opt(h.p90),
                format_duration_from_micros_opt(h.p99),
                format_duration_from_micros_opt(h.mean),
                format_duration_from_micros_opt(h.max),
                h.count
            )
            .ok();
        } else {
            out.push_str("  latency: n/a\n");
        }

        out.push('\n');
    }

    out.push_str("totals\n");
    writeln!(
        &mut out,
        "  requests: {} (failed {})",
        totals.requests_total, totals.failed_requests_total
    )
    .ok();
    writeln!(&mut out, "  iterations: {}", totals.iterations_total).ok();
    writeln!(
        &mut out,
        "  bytes: recv {} sent {}",
        format_bytes(totals.bytes_received_total),
        format_bytes(totals.bytes_sent_total)
    )
    .ok();

    if let Some(elapsed) = run_elapsed {
        let secs = elapsed.as_secs_f64().max(1e-9);
        let rps = (totals.requests_total as f64) / secs;
        let throughput_total = totals
            .bytes_received_total
            .saturating_add(totals.bytes_sent_total);
        let tps = (throughput_total as f64) / secs;

        writeln!(
            &mut out,
            "  rates: rps={} tps={}/s",
            format_rate(rps),
            format_bytes(tps.round() as u64)
        )
        .ok();
    }
    writeln!(
        &mut out,
        "  checks_failed_total: {}",
        totals.checks_failed_total
    )
    .ok();

    if let Some(series) = metric_series {
        render_checks(series, &mut out);
        render_metrics(series, &mut out);
    }

    out
}

#[derive(Default)]
struct Totals {
    requests_total: u64,
    failed_requests_total: u64,
    bytes_received_total: u64,
    bytes_sent_total: u64,
    iterations_total: u64,
    checks_failed_total: u64,
}

impl Totals {
    fn add(&mut self, s: &wrkr_core::ScenarioSummary) {
        self.requests_total = self.requests_total.saturating_add(s.requests_total);
        self.failed_requests_total = self
            .failed_requests_total
            .saturating_add(s.failed_requests_total);
        self.bytes_received_total = self
            .bytes_received_total
            .saturating_add(s.bytes_received_total);
        self.bytes_sent_total = self.bytes_sent_total.saturating_add(s.bytes_sent_total);
        self.iterations_total = self.iterations_total.saturating_add(s.iterations_total);
        self.checks_failed_total = self
            .checks_failed_total
            .saturating_add(s.checks_failed_total);
    }
}

fn render_checks(series: &[wrkr_core::MetricSeriesSummary], out: &mut String) {
    #[derive(Debug, Default, Clone, Copy)]
    struct Counts {
        pass: u64,
        fail: u64,
    }

    #[derive(Debug, Clone, PartialEq, Eq, Hash)]
    struct CheckKey {
        scenario: String,
        group: Option<String>,
        name: String,
        tags: Vec<(String, String)>,
    }

    let mut by_key: HashMap<CheckKey, Counts> = HashMap::new();

    for s in series.iter().filter(|s| s.name == "checks") {
        let wrkr_core::MetricValue::Counter(count) = &s.values else {
            continue;
        };

        let mut scenario = None;
        let mut group = None;
        let mut name = None;
        let mut status = None;

        for (k, v) in &s.tags {
            match k.as_str() {
                "scenario" => scenario = Some(v.clone()),
                "group" => group = Some(v.clone()),
                "name" => name = Some(v.clone()),
                "status" => status = Some(v.clone()),
                _ => {}
            }
        }

        let Some(scenario) = scenario else { continue };
        let Some(name) = name else { continue };
        let status = status.unwrap_or_else(|| "unknown".to_string());

        let tags = s
            .tags
            .iter()
            .filter(|(k, _)| !matches!(k.as_str(), "scenario" | "name" | "status"))
            .cloned()
            .collect::<Vec<_>>();

        let key = CheckKey {
            scenario,
            group,
            name,
            tags,
        };

        let entry = by_key.entry(key).or_default();
        if status == "pass" {
            entry.pass = entry.pass.saturating_add(*count);
        } else if status == "fail" {
            entry.fail = entry.fail.saturating_add(*count);
        }
    }

    if by_key.is_empty() {
        return;
    }

    out.push_str("\nchecks\n");

    let mut rows: Vec<(CheckKey, Counts)> = by_key.into_iter().collect();
    rows.sort_by(|(a, _), (b, _)| {
        a.scenario
            .cmp(&b.scenario)
            .then_with(|| a.group.cmp(&b.group))
            .then_with(|| a.name.cmp(&b.name))
            .then_with(|| a.tags.cmp(&b.tags))
    });

    let mut current_scenario: Option<String> = None;
    let mut current_group: Option<Option<String>> = None;

    for (k, c) in rows {
        if current_scenario.as_ref() != Some(&k.scenario) {
            current_scenario = Some(k.scenario.clone());
            current_group = None;
            writeln!(out, "scenario: {}", k.scenario).ok();
        }

        if current_group.as_ref() != Some(&k.group) {
            current_group = Some(k.group.clone());
            match &k.group {
                Some(g) => writeln!(out, "  group: {g}").ok(),
                None => writeln!(out, "  group: -").ok(),
            };
        }

        let tags_s = format_tags_inline(&k.tags, &[]);
        let status = if c.fail > 0 { "FAIL" } else { "OK" };

        if tags_s.is_empty() {
            writeln!(
                out,
                "    {}: pass={} fail={} [{status}]",
                k.name, c.pass, c.fail
            )
            .ok();
        } else {
            writeln!(
                out,
                "    {}{}: pass={} fail={} [{status}]",
                k.name, tags_s, c.pass, c.fail
            )
            .ok();
        }
    }
}

fn render_metrics(series: &[wrkr_core::MetricSeriesSummary], out: &mut String) {
    let mut by_scenario_group: BTreeMap<
        (String, Option<String>),
        Vec<&wrkr_core::MetricSeriesSummary>,
    > = BTreeMap::new();

    for s in series {
        if s.name == "checks" {
            continue;
        }

        let mut scenario = None;
        let mut group = None;
        for (k, v) in &s.tags {
            if k == "scenario" {
                scenario = Some(v.clone());
            } else if k == "group" {
                group = Some(v.clone());
            }
        }

        let scenario = scenario.unwrap_or_else(|| "global".to_string());
        by_scenario_group
            .entry((scenario, group))
            .or_default()
            .push(s);
    }

    if by_scenario_group.is_empty() {
        return;
    }

    out.push_str("\nmetrics\n");

    for ((scenario, group), mut rows) in by_scenario_group {
        writeln!(out, "scenario: {scenario}").ok();
        match group {
            Some(g) => writeln!(out, "  group: {g}").ok(),
            None => writeln!(out, "  group: -").ok(),
        };

        rows.sort_by(|a, b| a.name.cmp(&b.name).then_with(|| a.tags.cmp(&b.tags)));

        // Special-case: `vu_active` is a gauge that correctly ends at 0, which looks odd
        // in the final summary. If we also have `vu_active_max` for the same series, render
        // a combined line: `vu_active = end=0 peak=...` and hide the raw `vu_active_max`.
        let mut vu_active_end: HashMap<String, i64> = HashMap::new();
        let mut vu_active_peak: HashMap<String, i64> = HashMap::new();
        for s in &rows {
            let tags_s = format_tags_inline(&s.tags, &["scenario", "group"]);
            match (&s.name[..], &s.values) {
                ("vu_active", wrkr_core::MetricValue::Gauge(v)) => {
                    vu_active_end.insert(tags_s, *v);
                }
                ("vu_active_max", wrkr_core::MetricValue::Gauge(v)) => {
                    vu_active_peak.insert(tags_s, *v);
                }
                _ => {}
            }
        }

        for s in rows {
            let tags_s = format_tags_inline(&s.tags, &["scenario", "group"]);

            if s.name == "vu_active_max" && vu_active_end.contains_key(&tags_s) {
                continue;
            }

            if let ("vu_active", wrkr_core::MetricValue::Gauge(end), Some(peak)) =
                (&s.name[..], &s.values, vu_active_peak.get(&tags_s))
            {
                writeln!(out, "    {}{} = end={end} peak={peak}", s.name, tags_s).ok();
                continue;
            }

            match &s.values {
                wrkr_core::MetricValue::Counter(v) => {
                    writeln!(out, "    {}{} = {v}", s.name, tags_s).ok();
                }
                wrkr_core::MetricValue::Gauge(v) => {
                    writeln!(out, "    {}{} = {v}", s.name, tags_s).ok();
                }
                wrkr_core::MetricValue::Rate { total, hits, rate } => {
                    if let Some(rate) = rate {
                        writeln!(
                            out,
                            "    {}{} = hits={hits} total={total} rate={rate:.3}",
                            s.name, tags_s
                        )
                        .ok();
                    } else {
                        writeln!(out, "    {}{} = hits={hits} total={total}", s.name, tags_s).ok();
                    }
                }
                wrkr_core::MetricValue::Histogram(h) => {
                    writeln!(
                        out,
                        "    {}{} = p50={} p90={} p99={} mean={} max={} (n={})",
                        s.name,
                        tags_s,
                        format_duration_from_micros_opt(h.p50),
                        format_duration_from_micros_opt(h.p90),
                        format_duration_from_micros_opt(h.p99),
                        format_duration_from_micros_opt(h.mean),
                        format_duration_from_micros_opt(h.max),
                        h.count
                    )
                    .ok();
                }
            }
        }

        out.push('\n');
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn render_includes_scenario_and_totals() {
        let summary = wrkr_core::RunSummary {
            scenarios: vec![wrkr_core::ScenarioSummary {
                scenario: "default".to_string(),
                requests_total: 10,
                failed_requests_total: 2,
                bytes_received_total: 2048,
                bytes_sent_total: 1024,
                iterations_total: 10,
                checks_failed_total: 1,
                checks_failed: [("status_is_200".to_string(), 1)].into_iter().collect(),
                latency: None,
            }],
        };

        let text = render(&summary, None, Some(Duration::from_secs(10)));
        assert!(text.contains("scenario: default"));
        assert!(text.contains("requests: 10"));
        assert!(text.contains("failed 2"));
        assert!(text.contains("bytes: recv 2.00KiB sent 1.00KiB"));
        assert!(text.contains("checks_failed_total: 1"));
        assert!(text.contains("status_is_200: 1"));
        assert!(text.contains("latency: n/a"));
        assert!(text.contains("totals"));
        assert!(text.contains("rates: rps="));
        assert!(text.contains("tps="));
    }

    #[test]
    fn render_checks_includes_pass_fail_and_tags() {
        let summary = wrkr_core::RunSummary { scenarios: vec![] };

        let series = vec![
            wrkr_core::MetricSeriesSummary {
                name: "checks".to_string(),
                kind: wrkr_core::MetricKind::Counter,
                tags: vec![
                    ("scenario".to_string(), "Default".to_string()),
                    ("group".to_string(), "g1".to_string()),
                    ("name".to_string(), "status is 200".to_string()),
                    ("status".to_string(), "pass".to_string()),
                    ("region".to_string(), "eu".to_string()),
                ],
                values: wrkr_core::MetricValue::Counter(10),
            },
            wrkr_core::MetricSeriesSummary {
                name: "checks".to_string(),
                kind: wrkr_core::MetricKind::Counter,
                tags: vec![
                    ("scenario".to_string(), "Default".to_string()),
                    ("group".to_string(), "g1".to_string()),
                    ("name".to_string(), "status is 200".to_string()),
                    ("status".to_string(), "fail".to_string()),
                    ("region".to_string(), "eu".to_string()),
                ],
                values: wrkr_core::MetricValue::Counter(2),
            },
        ];

        let text = render(&summary, Some(&series), None);
        assert!(text.contains("checks"));
        assert!(text.contains("scenario: Default"));
        assert!(text.contains("group: g1"));
        assert!(text.contains("status is 200"));
        assert!(text.contains("pass=10"));
        assert!(text.contains("fail=2"));
        assert!(text.contains("region=eu"));
    }

    #[test]
    fn render_metrics_combines_vu_active_end_and_peak() {
        let summary = wrkr_core::RunSummary { scenarios: vec![] };

        let series = vec![
            wrkr_core::MetricSeriesSummary {
                name: "vu_active".to_string(),
                kind: wrkr_core::MetricKind::Gauge,
                tags: vec![
                    ("scenario".to_string(), "Default".to_string()),
                    ("group".to_string(), "g1".to_string()),
                ],
                values: wrkr_core::MetricValue::Gauge(0),
            },
            wrkr_core::MetricSeriesSummary {
                name: "vu_active_max".to_string(),
                kind: wrkr_core::MetricKind::Gauge,
                tags: vec![
                    ("scenario".to_string(), "Default".to_string()),
                    ("group".to_string(), "g1".to_string()),
                ],
                values: wrkr_core::MetricValue::Gauge(10),
            },
        ];

        let text = render(&summary, Some(&series), None);
        assert!(text.contains("vu_active = end=0 peak=10"));
        assert!(!text.contains("vu_active_max"));
    }
}
