use mlua::{Lua, Value};
use std::path::Path;
use std::sync::Arc;

use crate::Result;
use crate::loader::{chunk_name, configure_module_path};
use crate::modules;

pub struct HandleSummaryOutputs {
    pub stdout: Option<String>,
    pub stderr: Option<String>,
    pub files: Vec<(String, String)>,
}

fn init_lua(
    script_path: Option<&Path>,
    env_vars: &wrkr_core::runner::EnvVars,
    client: Arc<wrkr_core::HttpClient>,
    stats: Arc<wrkr_core::runner::RunStats>,
    shared: Arc<wrkr_core::runner::SharedStore>,
) -> Result<Lua> {
    let lua = Lua::new();
    configure_module_path(&lua, script_path)?;
    modules::register(
        &lua,
        modules::RegisterContext {
            script_path,
            env_vars,
            vu_id: 0,
            max_vus: 1,
            client,
            stats,
            shared,
        },
    )?;
    Ok(lua)
}

pub fn run_setup(
    script: &str,
    script_path: Option<&Path>,
    env_vars: &wrkr_core::runner::EnvVars,
    shared: Arc<wrkr_core::runner::SharedStore>,
) -> Result<()> {
    let lua = init_lua(
        script_path,
        env_vars,
        Arc::new(wrkr_core::HttpClient::default()),
        Arc::new(wrkr_core::runner::RunStats::default()),
        shared,
    )?;

    let chunk_name = chunk_name(script_path);
    lua.load(script).set_name(&chunk_name).exec()?;

    let globals = lua.globals();
    let setup: Option<mlua::Function> = globals.get("Setup").ok();
    let Some(setup) = setup else {
        return Ok(());
    };

    let _ignored: Value = setup.call(())?;
    Ok(())
}

pub fn run_teardown(
    script: &str,
    script_path: Option<&Path>,
    env_vars: &wrkr_core::runner::EnvVars,
    shared: Arc<wrkr_core::runner::SharedStore>,
) -> Result<()> {
    let lua = init_lua(
        script_path,
        env_vars,
        Arc::new(wrkr_core::HttpClient::default()),
        Arc::new(wrkr_core::runner::RunStats::default()),
        shared,
    )?;

    let chunk_name = chunk_name(script_path);
    lua.load(script).set_name(&chunk_name).exec()?;

    let globals = lua.globals();
    let teardown: Option<mlua::Function> = globals.get("Teardown").ok();
    let Some(teardown) = teardown else {
        return Ok(());
    };

    teardown.call::<()>(())?;
    Ok(())
}

pub fn run_handle_summary(
    script: &str,
    script_path: Option<&Path>,
    env_vars: &wrkr_core::runner::EnvVars,
    summary: &wrkr_core::runner::RunSummary,
    shared: Arc<wrkr_core::runner::SharedStore>,
) -> Result<Option<HandleSummaryOutputs>> {
    let lua = init_lua(
        script_path,
        env_vars,
        Arc::new(wrkr_core::HttpClient::default()),
        Arc::new(wrkr_core::runner::RunStats::default()),
        shared,
    )?;

    let chunk_name = chunk_name(script_path);
    lua.load(script).set_name(&chunk_name).exec()?;

    let globals = lua.globals();
    let handle_summary: Option<mlua::Function> = globals.get("HandleSummary").ok();
    let Some(handle_summary) = handle_summary else {
        return Ok(None);
    };

    let summary_tbl = lua.create_table()?;
    summary_tbl.set("requests_total", summary.requests_total)?;
    summary_tbl.set("checks_total", summary.checks_total)?;
    summary_tbl.set("checks_failed", summary.checks_failed)?;
    summary_tbl.set(
        "checks_succeeded",
        summary.checks_total.saturating_sub(summary.checks_failed),
    )?;

    let checks_tbl = lua.create_table()?;
    for (idx, check) in summary.checks_by_name.iter().enumerate() {
        let c = lua.create_table()?;
        c.set("name", check.name.as_str())?;
        c.set("total", check.total)?;
        c.set("failed", check.failed)?;
        c.set("succeeded", check.total.saturating_sub(check.failed))?;
        checks_tbl.set(idx + 1, c)?;
    }
    summary_tbl.set("checks_by_name", checks_tbl)?;

    summary_tbl.set("dropped_iterations_total", summary.dropped_iterations_total)?;
    summary_tbl.set("bytes_received_total", summary.bytes_received_total)?;
    summary_tbl.set("bytes_sent_total", summary.bytes_sent_total)?;
    summary_tbl.set("run_duration_ms", summary.run_duration_ms)?;
    summary_tbl.set("rps", summary.rps)?;
    summary_tbl.set("req_per_sec_avg", summary.req_per_sec_avg)?;
    summary_tbl.set("req_per_sec_stdev", summary.req_per_sec_stdev)?;
    summary_tbl.set("req_per_sec_max", summary.req_per_sec_max)?;
    summary_tbl.set("req_per_sec_stdev_pct", summary.req_per_sec_stdev_pct)?;
    summary_tbl.set("latency_p50_ms", summary.latency_p50_ms)?;
    summary_tbl.set("latency_p75_ms", summary.latency_p75_ms)?;
    summary_tbl.set("latency_p90_ms", summary.latency_p90_ms)?;
    summary_tbl.set("latency_p95_ms", summary.latency_p95_ms)?;
    summary_tbl.set("latency_p99_ms", summary.latency_p99_ms)?;
    summary_tbl.set("latency_mean_ms", summary.latency_mean_ms)?;
    summary_tbl.set("latency_stdev_ms", summary.latency_stdev_ms)?;
    summary_tbl.set("latency_max_ms", summary.latency_max_ms)?;

    let latency_dist_tbl = lua.create_table()?;
    for (idx, (p, v_ms)) in summary.latency_distribution_ms.iter().enumerate() {
        let t = lua.create_table()?;
        t.set("p", *p)?;
        t.set("ms", *v_ms)?;
        latency_dist_tbl.set(idx + 1, t)?;
    }
    summary_tbl.set("latency_distribution_ms", latency_dist_tbl)?;

    let metrics_tbl = lua.create_table()?;
    for (idx, series) in summary.metrics.iter().enumerate() {
        let s = lua.create_table()?;
        s.set("name", series.name.as_str())?;
        s.set(
            "type",
            match series.kind {
                wrkr_core::runner::MetricKind::Trend => "trend",
                wrkr_core::runner::MetricKind::Counter => "counter",
                wrkr_core::runner::MetricKind::Gauge => "gauge",
                wrkr_core::runner::MetricKind::Rate => "rate",
            },
        )?;

        let tags_tbl = lua.create_table()?;
        for (k, v) in &series.tags {
            tags_tbl.set(k.as_str(), v.as_str())?;
        }
        s.set("tags", tags_tbl)?;

        let values_tbl = lua.create_table()?;
        match &series.values {
            wrkr_core::runner::MetricValues::Trend {
                count,
                min,
                max,
                avg,
                p50,
                p90,
                p95,
                p99,
            } => {
                values_tbl.set("count", *count)?;
                values_tbl.set("min", *min)?;
                values_tbl.set("max", *max)?;
                values_tbl.set("avg", *avg)?;
                values_tbl.set("p50", *p50)?;
                values_tbl.set("p90", *p90)?;
                values_tbl.set("p95", *p95)?;
                values_tbl.set("p99", *p99)?;
            }
            wrkr_core::runner::MetricValues::Counter { value } => {
                values_tbl.set("value", *value)?;
            }
            wrkr_core::runner::MetricValues::Gauge { value } => {
                values_tbl.set("value", *value)?;
            }
            wrkr_core::runner::MetricValues::Rate { total, trues, rate } => {
                values_tbl.set("total", *total)?;
                values_tbl.set("trues", *trues)?;
                values_tbl.set("rate", *rate)?;
            }
        }
        s.set("values", values_tbl)?;

        metrics_tbl.set(idx + 1, s)?;
    }
    summary_tbl.set("metrics", metrics_tbl)?;

    let out: Value = handle_summary.call(summary_tbl)?;
    let Value::Table(out_tbl) = out else {
        return Ok(Some(HandleSummaryOutputs {
            stdout: None,
            stderr: None,
            files: Vec::new(),
        }));
    };

    let mut outputs = HandleSummaryOutputs {
        stdout: None,
        stderr: None,
        files: Vec::new(),
    };

    for pair in out_tbl.pairs::<Value, Value>() {
        let (k, v) = pair?;
        let key = match k {
            Value::String(s) => s.to_string_lossy().to_string(),
            _ => continue,
        };
        let value = match v {
            Value::String(s) => s.to_string_lossy().to_string(),
            _ => continue,
        };

        match key.as_str() {
            "stdout" => outputs.stdout = Some(value),
            "stderr" => outputs.stderr = Some(value),
            _ => outputs.files.push((key, value)),
        }
    }

    Ok(Some(outputs))
}
