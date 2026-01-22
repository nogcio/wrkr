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

pub struct RegisterRuntime<'a> {
    pub script_path: Option<&'a Path>,
    pub env_vars: &'a wrkr_core::runner::EnvVars,
    pub vu_id: u64,
    pub scenario: Arc<str>,
    pub client: Arc<wrkr_core::HttpClient>,
    pub stats: Arc<wrkr_core::runner::RunStats>,
    pub shared: Arc<wrkr_core::runner::SharedStore>,
}

pub fn register(lua: &Lua, rt: RegisterRuntime<'_>) -> Result<()> {
    http::register_runtime(
        lua,
        rt.scenario.clone(),
        rt.client.clone(),
        rt.stats.clone(),
    )?;
    grpc::register_runtime(lua, rt.scenario.clone(), rt.script_path, rt.stats.clone())?;
    check::register_runtime(lua, rt.scenario.clone(), rt.stats.clone())?;
    metrics::register_runtime(lua, rt.stats.clone())?;
    env::register_runtime(lua, rt.env_vars)?;
    fs::register(lua, rt.script_path)?;
    debug::register(lua)?;
    json::register(lua)?;
    uuid::register(lua)?;
    vu::register(lua, rt.vu_id)?;
    group::register(lua)?;
    shared::register_runtime(lua, rt.shared)?;
    wrkr::register(lua)?;
    Ok(())
}
