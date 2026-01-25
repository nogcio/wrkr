use std::sync::Arc;

use mlua::{Lua, Result, Table, Value};
use wrkr_metrics::MetricKind;

use super::preload_set;

mod record;

pub fn register(
    lua: &Lua,
    run_ctx: Arc<wrkr_core::RunScenariosContext>,
    metrics_ctx: wrkr_core::MetricsContext,
) -> Result<()> {
    let metrics = run_ctx.metrics.clone();
    let metric_checks = metrics.register("checks", MetricKind::Counter);

    let loader = lua.create_function(move |lua, ()| {
        let check_fn = {
            let metrics = metrics.clone();
            let metrics_ctx = metrics_ctx.clone();
            lua.create_function(move |lua, (data, checks): (Value, Table)| {
                record::run_checks(
                    lua,
                    data,
                    checks,
                    metrics.clone(),
                    metric_checks,
                    metrics_ctx.clone(),
                )
            })?
        };

        Ok(check_fn)
    })?;

    preload_set(lua, "wrkr/check", loader).map_err(mlua::Error::external)
}
