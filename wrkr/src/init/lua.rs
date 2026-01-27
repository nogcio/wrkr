use anyhow::Context as _;
use std::path::Path;

use crate::cli::InitArgs;

use super::fs::write_file;

pub async fn scaffold(root: &Path, args: &InitArgs) -> anyhow::Result<()> {
    write_luals_stubs(root, args.force).await?;
    write_luarc(root, args.force).await?;

    let script_name = args
        .script
        .as_deref()
        .unwrap_or_else(|| default_script_name());
    write_example_script(root, script_name, args.force).await?;

    if args.vscode {
        write_vscode_recommendations(root, args.force).await?;
    }

    Ok(())
}

fn default_script_name() -> &'static str {
    "script.lua"
}

async fn write_luals_stubs(root: &Path, force: bool) -> anyhow::Result<()> {
    let stubs_root = root.join(".wrkr").join("lua-stubs");
    tokio::fs::create_dir_all(&stubs_root)
        .await
        .with_context(|| format!("failed to create stubs dir: {}", stubs_root.display()))?;

    for stub in wrkr_lua::luals_stub_files() {
        let dst = stubs_root.join(stub.path);
        write_file(&dst, stub.contents, force).await?;
    }

    Ok(())
}

async fn write_luarc(root: &Path, force: bool) -> anyhow::Result<()> {
    let luarc = root.join(".luarc.json");
    write_file(&luarc, LUARC_JSON, force).await
}

async fn write_example_script(root: &Path, name: &str, force: bool) -> anyhow::Result<()> {
    let path = root.join(name);
    write_file(&path, EXAMPLE_SCRIPT_LUA, force).await
}

async fn write_vscode_recommendations(root: &Path, force: bool) -> anyhow::Result<()> {
    let dir = root.join(".vscode");
    tokio::fs::create_dir_all(&dir)
        .await
        .with_context(|| format!("failed to create dir: {}", dir.display()))?;

    write_file(&dir.join("extensions.json"), VSCODE_EXTENSIONS_JSON, force).await?;
    write_file(&dir.join("launch.json"), VSCODE_LAUNCH_JSON, force).await
}

const LUARC_JSON: &str = r#"{
  "$schema": "https://raw.githubusercontent.com/LuaLS/vscode-lua/master/setting/schema.json",
  "runtime.version": "Lua 5.4",
  "workspace.library": [
    ".wrkr/lua-stubs"
  ],
  "diagnostics.globals": [
    "Options",
    "Default",
    "Setup",
    "Teardown",
    "HandleSummary"
  ]
}
"#;

const VSCODE_EXTENSIONS_JSON: &str = r#"{
  "recommendations": [
    "sumneko.lua",
    "tomblind.local-lua-debugger-vscode"
  ]
}
"#;

const VSCODE_LAUNCH_JSON: &str = r#"{
  "version": "0.2.0",
  "inputs": [
    {
      "id": "wrkr_lua_script",
      "type": "promptString",
      "description": "Path to Lua script to run (relative to workspace)",
      "default": "script.lua"
    },
    {
      "id": "wrkr_base_url",
      "type": "promptString",
      "description": "BASE_URL for the script",
      "default": "https://example.com"
    }
  ],
  "configurations": [
    {
      "name": "Debug wrkr (Lua script)",
      "type": "lua-local",
      "request": "launch",
      "program": {
        "command": "wrkr",
        "communication": "pipe"
      },
      "cwd": "${workspaceFolder}",
      "args": [
        "run",
        "${input:wrkr_lua_script}",
        "--vus",
        "1",
        "--iterations",
        "1"
      ],
      "env": {
        "BASE_URL": "${input:wrkr_base_url}"
      },
      "scriptRoots": ["${workspaceFolder}"],
      "scriptFiles": ["**/*.lua"],
      "stopOnEntry": false,
      "verbose": false
    }
  ]
}
"#;

const EXAMPLE_SCRIPT_LUA: &str = r#"-- wrkr starter script
-- Run:
--   BASE_URL=https://example.com wrkr run script.lua

local http = require("wrkr/http")
local check = require("wrkr/check")
local env = require("wrkr/env")

Options = {
  vus = 10,
  duration = "10s",
}

function Default()
  local base = env.BASE_URL or "https://example.com"
  local res = http.get(base .. "/")

  check(res, {
    ["status is 2xx/3xx"] = function(r)
      return r.status and r.status >= 200 and r.status < 400
    end,
  })
end
"#;
