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
    //todo: populate summary_tbl with relevant data

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
