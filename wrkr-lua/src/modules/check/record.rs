use std::sync::Arc;

use mlua::{Lua, Table, Value};
use wrkr_metrics::{MetricHandle, MetricId, Registry};

pub(super) struct CheckRecorder {
    metrics: Arc<Registry>,
    metric_checks: MetricId,
    group: Option<String>,
    metrics_ctx: wrkr_core::MetricsContext,
}

impl CheckRecorder {
    pub(super) fn new(
        lua: &Lua,
        metrics: Arc<Registry>,
        metric_checks: MetricId,
        metrics_ctx: wrkr_core::MetricsContext,
    ) -> Self {
        let group = super::super::group::current_group(lua);
        Self {
            metrics,
            metric_checks,
            group,
            metrics_ctx,
        }
    }

    pub(super) fn record(&self, name: &str, passed: bool) {
        let status = if passed { "pass" } else { "fail" };

        let mut tags: Vec<(String, String)> = Vec::with_capacity(
            3 + self.metrics_ctx.scenario_tags().len() + if self.group.is_some() { 1 } else { 0 },
        );
        tags.push(("name".to_string(), name.to_string()));
        tags.push(("status".to_string(), status.to_string()));

        self.metrics_ctx
            .merge_base_tags_if_missing(&mut tags, &["group"]);

        if let Some(group) = self.group.as_deref()
            && !tags.iter().any(|(k, _)| k == "group")
        {
            tags.push(("group".to_string(), group.to_string()));
        }

        let tag_refs: Vec<(&str, &str)> =
            tags.iter().map(|(k, v)| (k.as_str(), v.as_str())).collect();
        let tags = self.metrics.resolve_tags(&tag_refs);

        if let Some(MetricHandle::Counter(c)) = self.metrics.get_handle(self.metric_checks, tags) {
            c.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        }
    }
}

pub(super) fn run_checks(
    lua: &Lua,
    data: Value,
    checks: Table,
    metrics: Arc<Registry>,
    metric_checks: MetricId,
    metrics_ctx: wrkr_core::MetricsContext,
) -> mlua::Result<bool> {
    let recorder = CheckRecorder::new(lua, metrics, metric_checks, metrics_ctx);

    let mut all_passed = true;

    // Iterate over the checks table: { "status is 200": function(v) return ... end }
    for pair in checks.pairs::<String, mlua::Function>() {
        let Ok((name, predicate)) = pair else {
            continue;
        };

        let passed = predicate.call::<bool>(data.clone()).unwrap_or_default();
        all_passed &= passed;

        recorder.record(name.as_str(), passed);
    }

    Ok(all_passed)
}
