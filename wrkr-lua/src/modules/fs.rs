use std::path::Path;

use mlua::{Lua, Table};

use crate::Result;
use crate::loader::read_script_relative_text;

pub(super) fn register(lua: &Lua, script_path: &Path) -> Result<()> {
    let script_path = script_path.to_path_buf();
    let loader = lua.create_function(move |lua, ()| {
        let t = lua.create_table()?;
        let script_path = script_path.clone();
        let read_file = lua.create_function(move |_lua, rel: String| {
            read_script_relative_text(script_path.as_path(), &rel).map_err(mlua::Error::external)
        })?;
        t.set("read_file", read_file)?;
        Ok::<Table, mlua::Error>(t)
    })?;

    super::preload_set(lua, "wrkr/fs", loader)
}
