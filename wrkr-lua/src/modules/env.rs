use std::sync::Arc;

use mlua::{Lua, Table};

use crate::Result;

pub(super) fn register_runtime(
    lua: &Lua,
    run_ctx: Arc<wrkr_core::RunScenariosContext>,
) -> Result<()> {
    let env_vars = run_ctx.env.clone();
    let loader = lua.create_function(move |lua, ()| {
        let t = lua.create_table()?;
        for (k, v) in env_vars.iter() {
            t.set(k.as_ref(), v.as_ref())?;
        }
        Ok::<Table, mlua::Error>(t)
    })?;
    super::preload_set(lua, "wrkr/env", loader)
}
