use std::path::Path;

use mlua::{Lua, Table};

use crate::Result;

mod client;
mod opts;
mod path;
mod result;

fn create_grpc_module(
    lua: &Lua,
    run_ctx: Arc<wrkr_core::RunScenariosContext>,
    metrics_ctx: wrkr_core::MetricsContext,
    script_path: &Path,
    max_vus: u64,
) -> Result<Table> {
    let grpc_tbl = lua.create_table()?;

    // grpc.Client.new(opts?) -> client
    let client_tbl = client::create_client_table(lua, run_ctx, metrics_ctx, script_path, max_vus)?;
    grpc_tbl.set("Client", client_tbl)?;

    Ok(grpc_tbl)
}

use std::sync::Arc;

pub(super) fn register_runtime(
    lua: &Lua,
    run_ctx: Arc<wrkr_core::RunScenariosContext>,
    metrics_ctx: wrkr_core::MetricsContext,
    script_path: &Path,
    max_vus: u64,
) -> Result<()> {
    let script_path = script_path.to_path_buf();
    let loader = {
        let run_ctx = run_ctx.clone();
        let metrics_ctx = metrics_ctx.clone();
        lua.create_function(move |lua, ()| {
            create_grpc_module(
                lua,
                run_ctx.clone(),
                metrics_ctx.clone(),
                &script_path,
                max_vus,
            )
            .map_err(mlua::Error::external)
        })?
    };

    super::preload_set(lua, "wrkr/grpc", loader)
}
