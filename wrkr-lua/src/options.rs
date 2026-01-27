use mlua::{Lua, Table, Value};
use std::sync::Arc;
use std::time::Duration;

use crate::loader::{chunk_name, configure_module_path};
use crate::modules;
use crate::{Error, Result};

pub fn parse_script_options(
    run_ctx: &wrkr_core::RunScenariosContext,
) -> Result<wrkr_core::ScriptOptions> {
    // Parse `Options`/`Options.scenarios` using a dedicated Lua state (no globals needed).
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
    let chunk_name = chunk_name(&run_ctx.script_path);
    lua.load(&run_ctx.script).set_name(&chunk_name).exec()?;

    let globals = lua.globals();
    let options: Option<Table> = globals.get("Options").ok();
    let scenarios_table: Option<Table> = options
        .as_ref()
        .and_then(|t| t.get::<Table>("scenarios").ok());

    let mut out = wrkr_core::ScriptOptions::default();
    if let Some(ref options) = options {
        out.vus = get_vus(options)?;
        out.iterations = get_iterations(options)?;
        out.duration = get_duration(options)?;
        out.thresholds = get_thresholds(options)?;
    }

    if let Some(scenarios_tbl) = scenarios_table {
        for pair in scenarios_tbl.pairs::<Value, Value>() {
            let (k, v) = pair?;
            let name = match k {
                Value::String(s) => s.to_string_lossy().to_string(),
                _ => continue,
            };
            let t = match v {
                Value::Table(t) => t,
                _ => continue,
            };

            let exec = t.get::<String>("exec").ok();
            let tags = get_scenario_tags(&t)?;
            let executor = get_string_any(&t, &["executor"])?;
            let vus = get_vus(&t)?;
            let iterations = get_iterations(&t)?;
            let duration = get_duration(&t)?;

            let start_vus = get_u64_any(&t, &["start_vus", "startVUs"], true)?;
            let start_rate = get_u64_any(&t, &["start_rate", "startRate"], true)?;
            let time_unit = get_duration_any(&t, &["time_unit", "timeUnit"])?;
            let pre_allocated_vus =
                get_u64_any(&t, &["pre_allocated_vus", "preAllocatedVUs"], false)?;
            let max_vus = get_u64_any(&t, &["max_vus", "maxVUs"], false)?;

            let stages = get_stages(&t)?;

            out.scenarios.push(wrkr_core::ScenarioOptions {
                name,
                exec,
                tags,
                executor,
                vus,
                iterations,
                duration,

                start_vus,
                stages,
                start_rate,
                time_unit,
                pre_allocated_vus,
                max_vus,
            });
        }
    }

    Ok(out)
}

fn get_thresholds(t: &Table) -> Result<Vec<wrkr_core::ThresholdSet>> {
    let v = match t.get::<Value>("thresholds") {
        Ok(v) => v,
        Err(_) => return Ok(Vec::new()),
    };

    let tbl = match v {
        Value::Nil => return Ok(Vec::new()),
        Value::Table(t) => t,
        _ => return Err(Error::InvalidThresholds),
    };

    let mut out = Vec::new();
    for pair in tbl.pairs::<Value, Value>() {
        let (k, v) = pair?;
        let metric_key = match k {
            Value::String(s) => s.to_string_lossy().to_string(),
            _ => continue,
        };

        let (metric, tags) = wrkr_core::parse_threshold_metric_key(&metric_key)
            .map_err(|_| Error::InvalidThresholds)?;

        let expressions: Vec<String> = match v {
            Value::String(s) => vec![s.to_string_lossy().to_string()],
            Value::Table(list) => {
                let mut exprs = Vec::new();
                for item in list.sequence_values::<Value>() {
                    let item = item?;
                    match item {
                        Value::String(s) => exprs.push(s.to_string_lossy().to_string()),
                        _ => return Err(Error::InvalidThresholds),
                    }
                }
                exprs
            }
            _ => return Err(Error::InvalidThresholds),
        };

        if expressions.is_empty() {
            return Err(Error::InvalidThresholds);
        }

        out.push(wrkr_core::ThresholdSet {
            metric,
            tags,
            expressions,
        });
    }

    Ok(out)
}

fn get_vus(t: &Table) -> Result<Option<u64>> {
    let v = match t.get::<Value>("vus") {
        Ok(v) => v,
        Err(_) => return Ok(None),
    };
    match v {
        Value::Nil => Ok(None),
        Value::Integer(i) if i > 0 => Ok(Some(i as u64)),
        Value::Number(n) if n.fract() == 0.0 && n > 0.0 => Ok(Some(n as u64)),
        _ => Err(Error::InvalidVus),
    }
}

fn get_iterations(t: &Table) -> Result<Option<u64>> {
    let v = match t.get::<Value>("iterations") {
        Ok(v) => v,
        Err(_) => return Ok(None),
    };
    match v {
        Value::Nil => Ok(None),
        Value::Integer(i) if i > 0 => Ok(Some(i as u64)),
        Value::Number(n) if n.fract() == 0.0 && n > 0.0 => Ok(Some(n as u64)),
        _ => Err(Error::InvalidIterations),
    }
}

fn get_duration(t: &Table) -> Result<Option<Duration>> {
    let v = match t.get::<Value>("duration") {
        Ok(v) => v,
        Err(_) => return Ok(None),
    };
    match v {
        Value::Nil => Ok(None),
        Value::Number(n) if n > 0.0 => Ok(Some(Duration::from_secs_f64(n))),
        Value::Integer(i) if i > 0 => Ok(Some(Duration::from_secs(i as u64))),
        Value::String(s) => {
            let s = s.to_string_lossy();
            humantime::parse_duration(&s)
                .map(Some)
                .map_err(|_| Error::InvalidDuration)
        }
        _ => Err(Error::InvalidDuration),
    }
}

fn get_string_any(t: &Table, keys: &[&str]) -> Result<Option<String>> {
    for key in keys {
        let v = match t.get::<Value>(*key) {
            Ok(v) => v,
            Err(_) => continue,
        };
        match v {
            Value::Nil => continue,
            Value::String(s) => return Ok(Some(s.to_string_lossy().to_string())),
            _ => return Err(Error::InvalidExecutor),
        }
    }
    Ok(None)
}

fn get_scenario_tags(t: &Table) -> Result<Vec<(String, String)>> {
    let v = match t.get::<Value>("tags") {
        Ok(v) => v,
        Err(_) => return Ok(Vec::new()),
    };

    let tbl = match v {
        Value::Nil => return Ok(Vec::new()),
        Value::Table(t) => t,
        _ => return Err(Error::InvalidScenarioTags),
    };

    let mut out = Vec::new();
    for pair in tbl.pairs::<Value, Value>() {
        let (k, v) = pair?;
        let k = match k {
            Value::String(s) => s.to_string_lossy().to_string(),
            _ => continue,
        };

        let v = match v {
            Value::String(s) => s.to_string_lossy().to_string(),
            Value::Integer(i) => i.to_string(),
            Value::Number(n) if n.is_finite() => n.to_string(),
            Value::Boolean(b) => b.to_string(),
            _ => continue,
        };

        out.push((k, v));
    }

    Ok(out)
}

fn get_duration_any(t: &Table, keys: &[&str]) -> Result<Option<Duration>> {
    for key in keys {
        let v = match t.get::<Value>(*key) {
            Ok(v) => v,
            Err(_) => continue,
        };
        match v {
            Value::Nil => continue,
            Value::Number(n) if n > 0.0 => return Ok(Some(Duration::from_secs_f64(n))),
            Value::Integer(i) if i > 0 => return Ok(Some(Duration::from_secs(i as u64))),
            Value::String(s) => {
                let s = s.to_string_lossy();
                return humantime::parse_duration(&s)
                    .map(Some)
                    .map_err(|_| Error::InvalidTimeUnit);
            }
            _ => return Err(Error::InvalidTimeUnit),
        }
    }
    Ok(None)
}

fn get_u64_any(t: &Table, keys: &[&str], allow_zero: bool) -> Result<Option<u64>> {
    for key in keys {
        let v = match t.get::<Value>(*key) {
            Ok(v) => v,
            Err(_) => continue,
        };

        match v {
            Value::Nil => continue,
            Value::Integer(i) => {
                if allow_zero {
                    if i >= 0 {
                        return Ok(Some(i as u64));
                    }
                } else if i > 0 {
                    return Ok(Some(i as u64));
                }
                return Err(Error::InvalidStages);
            }
            Value::Number(n) => {
                if n.fract() != 0.0 {
                    return Err(Error::InvalidStages);
                }
                if allow_zero {
                    if n >= 0.0 {
                        return Ok(Some(n as u64));
                    }
                } else if n > 0.0 {
                    return Ok(Some(n as u64));
                }
                return Err(Error::InvalidStages);
            }
            _ => return Err(Error::InvalidStages),
        }
    }
    Ok(None)
}

fn get_stages(t: &Table) -> Result<Vec<wrkr_core::Stage>> {
    let v = match t.get::<Value>("stages") {
        Ok(v) => v,
        Err(_) => return Ok(Vec::new()),
    };

    let tbl = match v {
        Value::Nil => return Ok(Vec::new()),
        Value::Table(t) => t,
        _ => return Err(Error::InvalidStages),
    };

    let mut out = Vec::new();
    for value in tbl.sequence_values::<Value>() {
        let value = value?;
        let stage_tbl = match value {
            Value::Table(t) => t,
            _ => return Err(Error::InvalidStages),
        };

        let duration = match get_duration(&stage_tbl)? {
            Some(d) => d,
            None => return Err(Error::InvalidStages),
        };

        // Stage targets allow 0 (e.g. ramp down to 0 VUs / 0 RPS).
        let target = match get_u64_any(&stage_tbl, &["target"], true)? {
            Some(v) => v,
            None => return Err(Error::InvalidStages),
        };

        out.push(wrkr_core::Stage { duration, target });
    }

    Ok(out)
}
