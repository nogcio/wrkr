use std::path::Path;
use std::sync::Arc;

use mlua::{Lua, Table};

use crate::Result;

mod check;
mod debug;
mod env;
mod fs;
mod group;
mod grpc;
mod http;
mod json;
mod metrics;
mod shared;
mod uuid;
mod vu;
mod wrkr;

fn preload_set(lua: &Lua, name: &str, loader: mlua::Function) -> Result<()> {
    let package: Table = lua.globals().get("package")?;
    let preload: Table = package.get("preload")?;
    preload.set(name, loader)?;
    Ok(())
}

pub fn register(
    lua: &Lua,
    script_path: Option<&Path>,
    env_vars: &wrkr_core::runner::EnvVars,
    vu_id: u64,
    client: Arc<wrkr_core::HttpClient>,
    stats: Arc<wrkr_core::runner::RunStats>,
    shared: Arc<wrkr_core::runner::SharedStore>,
) -> Result<()> {
    http::register_runtime(lua, client.clone(), stats.clone())?;
    grpc::register_runtime(lua, script_path, stats.clone())?;
    check::register_runtime(lua, stats.clone())?;
    metrics::register_runtime(lua, stats.clone())?;
    env::register_runtime(lua, env_vars)?;
    fs::register(lua, script_path)?;
    debug::register(lua)?;
    json::register(lua)?;
    uuid::register(lua)?;
    vu::register(lua, vu_id)?;
    group::register(lua)?;
    shared::register_runtime(lua, shared)?;
    wrkr::register(lua)?;
    Ok(())
}
