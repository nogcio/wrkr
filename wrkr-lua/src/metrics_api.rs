use mlua::{Lua, Table, UserData, UserDataMethods, Value};

use crate::group_api;
use crate::{Error, Result};

fn parse_tags(tags: Option<Table>) -> mlua::Result<Vec<(String, String)>> {
    let mut out = Vec::new();
    let Some(tags) = tags else {
        return Ok(out);
    };

    for pair in tags.pairs::<Value, Value>() {
        let (k, v) = pair?;
        let k = match k {
            Value::String(s) => s.to_string_lossy().to_string(),
            _ => continue,
        };
        let v = match v {
            Value::String(s) => s.to_string_lossy().to_string(),
            Value::Integer(i) => i.to_string(),
            Value::Number(n) => n.to_string(),
            Value::Boolean(b) => b.to_string(),
            _ => continue,
        };
        out.push((k, v));
    }

    Ok(out)
}

#[derive(Clone)]
struct LuaMetric {
    handle: wrkr_core::runner::MetricHandle,
}

impl UserData for LuaMetric {
    fn add_methods<M: UserDataMethods<Self>>(methods: &mut M) {
        methods.add_method("add", |lua, this, args: mlua::MultiValue| {
            match this.handle.kind() {
                wrkr_core::runner::MetricKind::Rate => {
                    // Rate.add(bool, tags?)
                    let mut it = args.into_iter();
                    let first = it
                        .next()
                        .ok_or_else(|| mlua::Error::external(Error::InvalidMetricValue))?;
                    let value = match first {
                        Value::Boolean(b) => b,
                        _ => return Err(mlua::Error::external(Error::InvalidMetricValue)),
                    };
                    let tags_tbl = it.next().and_then(|v| match v {
                        Value::Table(t) => Some(t),
                        Value::Nil => None,
                        _ => None,
                    });
                    let mut tags = parse_tags(tags_tbl)?;
                    if let Some(group) = group_api::current_group(lua)
                        && !tags.iter().any(|(k, _)| k == "group")
                    {
                        tags.push(("group".to_string(), group));
                    }
                    if tags.is_empty() {
                        this.handle.add_bool(value);
                    } else {
                        this.handle.add_bool_with_tags(value, &tags);
                    }
                    Ok(())
                }
                _ => {
                    // Trend/Counter/Gauge.add(number, tags?)
                    let mut it = args.into_iter();
                    let first = it
                        .next()
                        .ok_or_else(|| mlua::Error::external(Error::InvalidMetricValue))?;
                    let value = match first {
                        Value::Integer(i) => i as f64,
                        Value::Number(n) => n,
                        _ => return Err(mlua::Error::external(Error::InvalidMetricValue)),
                    };
                    let tags_tbl = it.next().and_then(|v| match v {
                        Value::Table(t) => Some(t),
                        Value::Nil => None,
                        _ => None,
                    });
                    let mut tags = parse_tags(tags_tbl)?;
                    if let Some(group) = group_api::current_group(lua)
                        && !tags.iter().any(|(k, _)| k == "group")
                    {
                        tags.push(("group".to_string(), group));
                    }
                    if tags.is_empty() {
                        this.handle.add(value);
                    } else {
                        this.handle.add_with_tags(value, &tags);
                    }
                    Ok(())
                }
            }
        });
    }
}

pub fn create_metrics_module(
    lua: &Lua,
    stats: std::sync::Arc<wrkr_core::runner::RunStats>,
) -> Result<Table> {
    let t = lua.create_table()?;

    let mk = |kind: wrkr_core::runner::MetricKind| {
        let stats = stats.clone();
        lua.create_function(move |lua, name: String| {
            if name.trim().is_empty() {
                return Err(mlua::Error::external(Error::InvalidMetricName));
            }
            let handle = stats.metric_handle(kind, &name);
            lua.create_userdata(LuaMetric { handle })
        })
    };

    t.set("Trend", mk(wrkr_core::runner::MetricKind::Trend)?)?;
    t.set("Counter", mk(wrkr_core::runner::MetricKind::Counter)?)?;
    t.set("Gauge", mk(wrkr_core::runner::MetricKind::Gauge)?)?;
    t.set("Rate", mk(wrkr_core::runner::MetricKind::Rate)?)?;

    Ok(t)
}
