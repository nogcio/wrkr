use std::path::Path;
use std::sync::Arc;

use mlua::{Lua, Table};

use crate::Result;

mod debug;
mod env;
mod fs;
mod check;
mod group;
mod grpc;
mod http;
mod json;
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

pub struct RegisterContext<'a> {
    pub script_path: Option<&'a Path>,
    pub env_vars: &'a wrkr_core::runner::EnvVars,
    pub vu_id: u64,
    pub max_vus: u64,
    pub client: Arc<wrkr_core::HttpClient>,
    pub shared: Arc<wrkr_core::runner::SharedStore>,
    pub metrics: Arc<wrkr_metrics::Registry>,
}

pub fn register(lua: &Lua, ctx: RegisterContext<'_>) -> Result<()> {
    http::register_runtime(lua, ctx.client.clone())?;
    grpc::register_runtime(lua, ctx.script_path, ctx.max_vus)?;
    env::register_runtime(lua, ctx.env_vars)?;
    check::register(lua, ctx.metrics)?;
    fs::register(lua, ctx.script_path)?;
    debug::register(lua)?;
    json::register(lua)?;
    uuid::register(lua)?;
    vu::register(lua, ctx.vu_id)?;
    group::register(lua)?;
    shared::register_runtime(lua, ctx.shared)?;
    wrkr::register(lua)?;
    Ok(())
}
