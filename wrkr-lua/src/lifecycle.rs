use crate::Result;
use crate::loader::{chunk_name, configure_module_path};
use crate::modules;
use mlua::{Lua, Value};
use std::sync::Arc;

pub struct HandleSummaryOutputs {
    pub stdout: Option<String>,
    pub stderr: Option<String>,
    pub files: Vec<(String, String)>,
}

fn init_lua(run_ctx: &wrkr_core::RunScenariosContext) -> Result<Lua> {
    let lua = Lua::new();
    configure_module_path(&lua, &run_ctx.script_path)?;
    modules::register(
        &lua,
        modules::RegisterContext {
            vu_id: 0,
            max_vus: 1,
            metrics_ctx: wrkr_core::MetricsContext::new(
                Arc::from("Default"),
                Arc::<[(String, String)]>::from([]),
            ),
            run_ctx,
        },
    )?;
    Ok(lua)
}

pub fn run_setup(run_ctx: &wrkr_core::RunScenariosContext) -> Result<()> {
    let lua = init_lua(run_ctx)?;

    let chunk_name = chunk_name(&run_ctx.script_path);
    lua.load(&run_ctx.script).set_name(&chunk_name).exec()?;

    let globals = lua.globals();
    let setup: Option<mlua::Function> = globals.get("Setup").ok();
    let Some(setup) = setup else {
        return Ok(());
    };

    let _ignored: Value = setup.call(())?;
    Ok(())
}

pub fn run_teardown(run_ctx: &wrkr_core::RunScenariosContext) -> Result<()> {
    let lua = init_lua(run_ctx)?;

    let chunk_name = chunk_name(&run_ctx.script_path);
    lua.load(&run_ctx.script).set_name(&chunk_name).exec()?;

    let globals = lua.globals();
    let teardown: Option<mlua::Function> = globals.get("Teardown").ok();
    let Some(teardown) = teardown else {
        return Ok(());
    };

    teardown.call::<()>(())?;
    Ok(())
}

pub fn run_handle_summary(
    run_ctx: &wrkr_core::RunScenariosContext,
    summary: &wrkr_core::RunSummary,
) -> Result<Option<HandleSummaryOutputs>> {
    let lua = init_lua(run_ctx)?;

    let chunk_name = chunk_name(&run_ctx.script_path);
    lua.load(&run_ctx.script).set_name(&chunk_name).exec()?;

    let globals = lua.globals();
    let handle_summary: Option<mlua::Function> = globals.get("HandleSummary").ok();
    let Some(handle_summary) = handle_summary else {
        return Ok(None);
    };

    let summary_tbl = lua.create_table()?;

    let mut requests_total = 0u64;
    let mut failed_requests_total = 0u64;
    let mut bytes_received_total = 0u64;
    let mut bytes_sent_total = 0u64;
    let mut iterations_total = 0u64;
    let mut checks_failed_total = 0u64;

    let checks_failed_tbl = lua.create_table()?;
    let scenarios_tbl = lua.create_table()?;

    for (idx, s) in summary.scenarios.iter().enumerate() {
        requests_total = requests_total.saturating_add(s.requests_total);
        failed_requests_total = failed_requests_total.saturating_add(s.failed_requests_total);
        bytes_received_total = bytes_received_total.saturating_add(s.bytes_received_total);
        bytes_sent_total = bytes_sent_total.saturating_add(s.bytes_sent_total);
        iterations_total = iterations_total.saturating_add(s.iterations_total);
        checks_failed_total = checks_failed_total.saturating_add(s.checks_failed_total);

        for (name, count) in &s.checks_failed {
            let cur: Option<u64> = checks_failed_tbl.get(name.as_str()).ok();
            checks_failed_tbl.set(name.as_str(), cur.unwrap_or(0).saturating_add(*count))?;
        }

        let scenario_tbl = lua.create_table()?;
        scenario_tbl.set("scenario", s.scenario.as_str())?;
        scenario_tbl.set("requests_total", s.requests_total)?;
        scenario_tbl.set("failed_requests_total", s.failed_requests_total)?;
        scenario_tbl.set("bytes_received_total", s.bytes_received_total)?;
        scenario_tbl.set("bytes_sent_total", s.bytes_sent_total)?;
        scenario_tbl.set("iterations_total", s.iterations_total)?;
        scenario_tbl.set("checks_failed_total", s.checks_failed_total)?;

        let scenario_checks_failed_tbl = lua.create_table()?;
        for (name, count) in &s.checks_failed {
            scenario_checks_failed_tbl.set(name.as_str(), *count)?;
        }
        scenario_tbl.set("checks_failed", scenario_checks_failed_tbl)?;

        if let Some(lat) = &s.latency {
            let latency_tbl = lua.create_table()?;
            latency_tbl.set("p50", lat.p50)?;
            latency_tbl.set("p75", lat.p75)?;
            latency_tbl.set("p90", lat.p90)?;
            latency_tbl.set("p95", lat.p95)?;
            latency_tbl.set("p99", lat.p99)?;
            latency_tbl.set("min", lat.min)?;
            latency_tbl.set("max", lat.max)?;
            latency_tbl.set("mean", lat.mean)?;
            latency_tbl.set("stdev", lat.stdev)?;
            latency_tbl.set("count", lat.count)?;
            scenario_tbl.set("latency", latency_tbl)?;
        }

        scenarios_tbl.set(idx + 1, scenario_tbl)?;
    }

    summary_tbl.set("requests_total", requests_total)?;
    summary_tbl.set("failed_requests_total", failed_requests_total)?;
    summary_tbl.set("bytes_received_total", bytes_received_total)?;
    summary_tbl.set("bytes_sent_total", bytes_sent_total)?;
    summary_tbl.set("iterations_total", iterations_total)?;
    summary_tbl.set("checks_failed_total", checks_failed_total)?;
    summary_tbl.set("checks_failed", checks_failed_tbl)?;
    summary_tbl.set("scenarios", scenarios_tbl)?;

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
