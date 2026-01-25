use std::sync::Arc;

use mlua::{Lua, Table};

use crate::Result;

mod check;
mod debug;
mod env;
mod fs;
mod group;
#[cfg(feature = "grpc")]
mod grpc;
#[cfg(feature = "http")]
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

pub struct RegisterContext<'a> {
    pub vu_id: u64,
    pub max_vus: u64,
    pub metrics_ctx: wrkr_core::MetricsContext,
    pub run_ctx: &'a wrkr_core::RunScenariosContext,
}

pub fn register(lua: &Lua, ctx: RegisterContext<'_>) -> Result<()> {
    let run_ctx = Arc::new(ctx.run_ctx.clone());
    let metrics_ctx = ctx.metrics_ctx;

    metrics::register_runtime(lua, run_ctx.clone(), metrics_ctx.clone())?;

    #[cfg(feature = "http")]
    http::register_runtime(lua, run_ctx.clone(), metrics_ctx.clone())?;

    #[cfg(feature = "grpc")]
    grpc::register_runtime(
        lua,
        run_ctx.clone(),
        metrics_ctx.clone(),
        &ctx.run_ctx.script_path,
        ctx.max_vus,
    )?;

    env::register_runtime(lua, run_ctx.clone())?;
    check::register(lua, run_ctx.clone(), metrics_ctx.clone())?;
    fs::register(lua, &ctx.run_ctx.script_path)?;
    debug::register(lua)?;
    json::register(lua)?;
    uuid::register(lua)?;
    vu::register(lua, ctx.vu_id)?;
    group::register(lua)?;
    shared::register_runtime(lua, run_ctx.clone())?;
    wrkr::register(lua)?;
    Ok(())
}
