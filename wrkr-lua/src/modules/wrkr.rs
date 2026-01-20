use mlua::{Lua, Table};

use crate::Result;

pub(super) fn register(lua: &Lua) -> Result<()> {
    let loader = lua.create_function(|lua, ()| {
        let t = lua.create_table()?;

        let require: mlua::Function = lua.globals().get("require")?;
        let http: Table = require.call("wrkr/http")?;
        let grpc: Table = require.call("wrkr/grpc")?;
        let check: mlua::Function = require.call("wrkr/check")?;
        let env: Table = require.call("wrkr/env")?;
        let fs: Table = require.call("wrkr/fs")?;
        let group: Table = require.call("wrkr/group")?;
        let json: Table = require.call("wrkr/json")?;
        let uuid: Table = require.call("wrkr/uuid")?;
        let metrics: Table = require.call("wrkr/metrics")?;
        let shared: Table = require.call("wrkr/shared")?;
        let vu: Table = require.call("wrkr/vu")?;

        t.set("http", http)?;
        t.set("grpc", grpc)?;
        t.set("check", check)?;
        t.set("env", env)?;
        t.set("fs", fs)?;
        t.set("group", group)?;
        t.set("json", json)?;
        t.set("uuid", uuid)?;
        t.set("metrics", metrics)?;
        t.set("shared", shared)?;
        t.set("vu", vu)?;
        Ok::<Table, mlua::Error>(t)
    })?;

    super::preload_set(lua, "wrkr", loader)
}
