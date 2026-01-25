use std::sync::Arc;

use mlua::{Lua, Result, Table, Value};
use wrkr_metrics::{MetricHandle, MetricKind, Registry};

use super::preload_set;

pub fn register(lua: &Lua, metrics: Arc<Registry>) -> Result<()> {
    // Define the 'wrkr/check' module loader function
    let check_module_loader = lua.create_function(move |lua, ()| {
        let metrics = metrics.clone();
        
        // Define the 'check' function
        let check_fn = lua.create_function(move |lua, (data, checks): (Value, Table)| {
            let mut all_passed = true;
            
            // Allow check('name', val, checks) style or check(val, checks)
            // But usually k6 style is check(val, checks)
            // If the first arg is just data, we use it.
            
            // Register the 'checks' metric if not already
            let metric_checks = metrics.register("checks", MetricKind::Counter);

            // Iterate over the checks table: { "status is 200": fn(r) ... }
            for pair in checks.pairs::<String, mlua::Function>() {
                if let Ok((name, predicate)) = pair {
                    let passed = predicate.call::<bool>(data.clone()).unwrap_or(false);
                    if !passed {
                        all_passed = false;
                    }

                    let status = if passed { "pass" } else { "fail" };
                    let tags = metrics.resolve_tags(&[("name", &name), ("status", status)]);

                    if let Some(MetricHandle::Counter(c)) = metrics.get_handle(metric_checks, tags) {
                        c.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                    }
                }
            }
            
            Ok(all_passed)
        })?;

        Ok(check_fn)
    })?;

    preload_set(lua, "wrkr/check", check_module_loader).map_err(mlua::Error::external)
}
