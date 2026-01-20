use std::sync::atomic::{AtomicBool, Ordering};

use mlua::Lua;

use crate::Result;

static DEBUGGER_STARTED: AtomicBool = AtomicBool::new(false);

fn env_truthy(key: &str) -> bool {
    matches!(
        std::env::var(key).ok().as_deref(),
        Some("1") | Some("true") | Some("TRUE") | Some("yes") | Some("YES")
    )
}

fn should_auto_start() -> bool {
    // Set by `tomblind.local-lua-debugger-vscode` when running under VS Code.
    env_truthy("LOCAL_LUA_DEBUGGER_VSCODE") || env_truthy("WRKR_LUA_DEBUG")
}

pub fn debugging_enabled() -> bool {
    should_auto_start()
}

fn start_in_lua(lua: &Lua) -> Result<bool> {
    let debugger_filepath = std::env::var("LOCAL_LUA_DEBUGGER_FILEPATH").ok();

    // Best-effort: never fail the run if debugger can't start.
    // This tries to load the extension-provided file path when present, then `require("lldebugger")`.
    let chunk = r#"
        local fp = ...

        if type(fp) == "string" and fp ~= "" then
          local ok_loadfile, lf = pcall(function()
            return loadfile
          end)
          if ok_loadfile and type(lf) == "function" then
            local ok_mod, mod = pcall(lf, fp)
            if ok_mod and type(mod) == "function" then
              local ok_eval, dbg = pcall(mod)
              if ok_eval and dbg ~= nil then
                package.loaded["lldebugger"] = dbg
              end
            end
          end
        end

        local ok_req, dbg = pcall(require, "lldebugger")
        if not ok_req then
          _G._wrkr_debugger_error = tostring(dbg)
          return false
        end
        if type(dbg) ~= "table" then
          return false
        end

        -- Keep a convenient global, like the extension does.
        lldebugger = dbg

        if type(dbg.start) == "function" then
          pcall(dbg.start)
          return true
        end

        return false
    "#;

    let res = lua
        .load(chunk)
        .set_name("wrkr_lua_debugger")
        .call::<bool>(debugger_filepath.unwrap_or_default());

    match res {
        Ok(started) => Ok(started),
        Err(_) => Ok(false),
    }
}

pub fn maybe_start_debugger(lua: &Lua) {
    if !should_auto_start() {
        return;
    }

    // Attach debugger at most once per process.
    // This avoids multiple debuggers competing for the same pipe.
    if DEBUGGER_STARTED.swap(true, Ordering::SeqCst) {
        return;
    }

    // Best-effort: never fail the run if debugger can't start.
    let _ = start_in_lua(lua);
}

pub fn start_debugger(lua: &Lua) -> Result<bool> {
    start_in_lua(lua)
}
