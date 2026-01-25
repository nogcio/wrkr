use std::sync::Arc;

use mlua::{Lua, Table, Value};
use wrkr_metrics::MetricKind;

use crate::Result;

mod opts;

use opts::{MetricAddLuaArgs, resolve_tags};

fn make_metric_handle_table(
    lua: &Lua,
    metrics: Arc<wrkr_metrics::Registry>,
    metric: wrkr_metrics::MetricId,
    kind: MetricKind,
    metrics_ctx: wrkr_core::MetricsContext,
) -> Result<Table> {
    let t = lua.create_table()?;

    let add = {
        let metrics = metrics.clone();
        let metrics_ctx = metrics_ctx.clone();
        lua.create_function(
            move |lua, (_this, value, tags): (Table, Value, Option<Table>)| {
                let args = MetricAddLuaArgs::parse(lua, &metrics_ctx, value, tags)?;
                let tags = resolve_tags(&metrics, &args.tags);

                let Some(handle) = metrics.get_handle(metric, tags) else {
                    return Ok(());
                };

                match (kind, args.value) {
                    (MetricKind::Counter, Value::Integer(i)) => {
                        let i: u64 = i
                            .try_into()
                            .map_err(|_| mlua::Error::external(crate::Error::InvalidMetricValue))?;
                        handle.increment(i);
                        Ok(())
                    }
                    (MetricKind::Counter, Value::Number(n)) => {
                        if !n.is_finite() || n < 0.0 || n.fract() != 0.0 || n > u64::MAX as f64 {
                            return Err(mlua::Error::external(crate::Error::InvalidMetricValue));
                        }
                        handle.increment(n as u64);
                        Ok(())
                    }

                    (MetricKind::Gauge, Value::Integer(i)) => {
                        handle.increment_gauge(i);
                        Ok(())
                    }
                    (MetricKind::Gauge, Value::Number(n)) => {
                        if !n.is_finite()
                            || n.fract() != 0.0
                            || n < i64::MIN as f64
                            || n > i64::MAX as f64
                        {
                            return Err(mlua::Error::external(crate::Error::InvalidMetricValue));
                        }
                        handle.increment_gauge(n as i64);
                        Ok(())
                    }

                    (MetricKind::Rate, Value::Boolean(b)) => {
                        let hits = if b { 1 } else { 0 };
                        handle.add_rate(hits, 1);
                        Ok(())
                    }

                    (MetricKind::Histogram, Value::Integer(i)) => {
                        let i: u64 = i
                            .try_into()
                            .map_err(|_| mlua::Error::external(crate::Error::InvalidMetricValue))?;
                        handle.observe_histogram(i);
                        Ok(())
                    }
                    (MetricKind::Histogram, Value::Number(n)) => {
                        if !n.is_finite() || n < 0.0 || n > u64::MAX as f64 {
                            return Err(mlua::Error::external(crate::Error::InvalidMetricValue));
                        }
                        handle.observe_histogram(n.round() as u64);
                        Ok(())
                    }

                    _ => Err(mlua::Error::external(crate::Error::InvalidMetricValue)),
                }
            },
        )?
    };

    t.set("add", add)?;
    Ok(t)
}

pub(super) fn register_runtime(
    lua: &Lua,
    run_ctx: Arc<wrkr_core::RunScenariosContext>,
    metrics_ctx: wrkr_core::MetricsContext,
) -> Result<()> {
    let metrics = run_ctx.metrics.clone();

    let loader = {
        let metrics = metrics.clone();
        lua.create_function(move |lua, ()| {
            let t = lua.create_table()?;

            let make = |kind: MetricKind| {
                let metrics = metrics.clone();
                let metrics_ctx = metrics_ctx.clone();
                lua.create_function(move |lua, name: String| {
                    if name.trim().is_empty() {
                        return Err(mlua::Error::external(crate::Error::InvalidMetricName));
                    }

                    let id = metrics.register(&name, kind);
                    make_metric_handle_table(lua, metrics.clone(), id, kind, metrics_ctx.clone())
                        .map_err(mlua::Error::external)
                })
            };

            t.set("Trend", make(MetricKind::Histogram)?)?;
            t.set("Counter", make(MetricKind::Counter)?)?;
            t.set("Gauge", make(MetricKind::Gauge)?)?;
            t.set("Rate", make(MetricKind::Rate)?)?;

            Ok::<_, mlua::Error>(t)
        })?
    };

    super::preload_set(lua, "wrkr/metrics", loader)
}
