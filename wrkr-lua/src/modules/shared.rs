use std::sync::Arc;

use mlua::{Lua, Value};

pub(super) fn register_runtime(
    lua: &Lua,
    shared: Arc<wrkr_core::runner::SharedStore>,
) -> crate::Result<()> {
    let loader = {
        let shared = shared.clone();
        lua.create_function(move |lua, ()| {
            let t = lua.create_table()?;

            let get = {
                let shared = shared.clone();
                lua.create_function(move |lua, key: String| {
                    let Some(value) = shared.get(&key) else {
                        return Ok(Value::Nil);
                    };
                    crate::json_util::from_shared_value(lua, &value).map_err(mlua::Error::external)
                })?
            };

            let set = {
                let shared = shared.clone();
                lua.create_function(move |lua, (key, value): (String, Value)| {
                    let value = crate::json_util::to_shared_value(lua, value)
                        .map_err(mlua::Error::external)?;
                    shared.set(&key, value);
                    Ok(())
                })?
            };

            let delete = {
                let shared = shared.clone();
                lua.create_function(move |_lua, key: String| {
                    shared.delete(&key);
                    Ok(())
                })?
            };

            let incr = {
                let shared = shared.clone();
                lua.create_function(move |_lua, (key, delta): (String, Option<i64>)| {
                    let delta = delta.unwrap_or(1);
                    Ok(shared.incr(&key, delta))
                })?
            };

            let counter = {
                let shared = shared.clone();
                lua.create_function(move |_lua, key: String| Ok(shared.get_counter(&key)))?
            };

            let wait = {
                let shared = shared.clone();
                lua.create_async_function(move |lua, key: String| {
                    let shared = shared.clone();
                    async move {
                        shared.wait_for_key(&key).await;
                        let Some(value) = shared.get(&key) else {
                            return Ok(Value::Nil);
                        };
                        crate::json_util::from_shared_value(&lua, &value)
                            .map_err(mlua::Error::external)
                    }
                })?
            };

            let barrier = {
                let shared = shared.clone();
                lua.create_async_function(move |_lua, (name, parties): (String, u64)| {
                    let shared = shared.clone();
                    async move {
                        let parties = parties.min(usize::MAX as u64) as usize;
                        shared
                            .barrier_wait(&name, parties)
                            .await
                            .map_err(mlua::Error::external)?;
                        Ok(())
                    }
                })?
            };

            t.set("get", get)?;
            t.set("set", set)?;
            t.set("delete", delete)?;
            t.set("incr", incr)?;
            t.set("counter", counter)?;
            t.set("wait", wait)?;
            t.set("barrier", barrier)?;

            Ok::<_, mlua::Error>(t)
        })?
    };

    super::preload_set(lua, "wrkr/shared", loader)
}
