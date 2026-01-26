# wrkr-lua

This crate embeds Lua (via `mlua`) and exposes a small runtime API to scripts via `require("wrkr/... ")` modules.

## Script shape

- Define optional global `options` for shared defaults or per-scenario overrides.
- Export an entrypoint function named `Default()`. Any custom scenarios should set `exec` to the function name they want to run.
- Optionally define lifecycle hooks `Setup()`, `Teardown()`, and `HandleSummary(summary)`.

Example:

```lua
options = { iterations = 1 }

function Default()
  -- your scenario here
end

-- Optional lifecycle hooks
function Setup()
  -- Called once before running scenarios (best-effort).
end

function Teardown()
  -- Called once after running scenarios (best-effort).
end

function HandleSummary(summary)
  -- Called once after Teardown.
  -- Return a table like: { stdout = "...", stderr = "...", ["out.txt"] = "..." }
  -- Output files are written relative to the current working directory.
  return {}
end
```

## Scenarios and executors

`wrkr` supports multiple scenarios via `options.scenarios`.

### `constant-vus` (default)

If `executor` is omitted, the scenario runs with a constant number of VUs (current default behavior).

```lua
options = {
  scenarios = {
    main = { executor = "constant-vus", vus = 10, duration = "10s", exec = "Default" },
  },
}
```

### `ramping-vus`

Ramp the number of active VUs up/down over time.

```lua
options = {
  scenarios = {
    main = {
      executor = "ramping-vus",
      startVUs = 0,
      stages = {
        { duration = "10s", target = 50 },
        { duration = "10s", target = 0 },
      },
      exec = "Default",
    },
  },
}
```

### `ramping-arrival-rate`

Ramp an open-model arrival rate (iterations started per `timeUnit`), with adaptive VU activation.

```lua
options = {
  scenarios = {
    main = {
      executor = "ramping-arrival-rate",
      startRate = 10,
      timeUnit = "1s",
      preAllocatedVUs = 10,
      maxVUs = 200,
      stages = {
        { duration = "10s", target = 100 },
        { duration = "10s", target = 10 },
      },
      exec = "Default",
    },
  },
}
```

## Runtime modules

- `require("wrkr/http")`
  - `http.get(url, opts?) -> { status, body, error? }`
  - `http.post(url, body, opts?) -> { status, body, error? }`
    - If `body` is a Lua string, it is sent as-is.
    - Otherwise, `body` is JSON-encoded (using `wrkr/json`) and sent with `Content-Type: application/json; charset=utf-8` unless overridden.
  - `opts.headers`: table of headers
  - `opts.params`: table of query params
  - `opts.timeout`: number (seconds) or duration string (e.g. `"250ms"`, `"10s"`)

- `require("wrkr/check")`
  - `check(res, { ["name"] = function(res) return bool end, ... }) -> bool`

- `require("wrkr/env")`
  - table of environment variables (string keys/values)

- `require("wrkr/json")`
  - `json.encode(value) -> string`
  - `json.decode(string) -> any`

- `require("wrkr/vu")`
  - `vu.id() -> integer` (stable numeric VU id; `0` during the options-parsing phase)

- `require("wrkr/shared")`
  - `shared.get(key) -> any|nil`
  - `shared.set(key, value) -> nil` (value is JSON-encoded)
  - `shared.delete(key) -> nil`
  - `shared.incr(key, delta?) -> integer`
  - `shared.counter(key) -> integer`
  - `shared.wait(key) -> any` (async)
  - `shared.barrier(name, parties) -> nil` (async)

- `require("wrkr/fs")`
  - `fs.read_file(rel) -> string`
  - Reads UTF-8 text relative to the script path.

- `require("wrkr")`
  - Convenience table aggregating the modules above.

## Local modules (vendoring)

Scripts can `require()` modules located next to the script file. The runner prepends the script directory to `package.path`:

- `${SCRIPT_DIR}/?.lua`
- `${SCRIPT_DIR}/?/init.lua`

Example:

- `benchmark.lua`
- `lib/checks.lua` -> `require("lib.checks")`

## LuaRocks / external dependencies (optional)

If you want to use LuaRocks-installed packages, set `LUA_PATH`/`LUA_CPATH` (or `WRKR_LUA_PATH`/`WRKR_LUA_CPATH`).

The loader prepends these env vars to `package.path`/`package.cpath` if present.

Typical workflow:

- `luarocks --tree lua_modules install <rock>`
- `eval "$(luarocks --tree lua_modules path)"`
- Run `wrkr` as usual.

## Editor experience (LuaLS)

This repo includes LuaLS type stubs for the `wrkr/*` modules under `wrkr-lua/lua-stubs`, and a root `.luarc.json` to enable autocomplete in editors using Lua Language Server.

If you're an end user installing only the `wrkr` binary (without this repo), run:

```bash
wrkr init --lang lua
```

This scaffolds a small script workspace in the current directory including `.luarc.json` and `.wrkr/lua-stubs/`.

## Debugging Lua scripts in VS Code

Recommended setup:

1. Install VS Code extensions:
  - `sumneko.lua`
  - `tomblind.local-lua-debugger-vscode`

2. Use the workspace debug config "Debug wrkr (Lua script)".

How it works:

- The debug extension launches the `wrkr` process and sets env vars like `LOCAL_LUA_DEBUGGER_VSCODE=1`.
- The runner detects this and starts `lldebugger` automatically (best-effort) before executing your script.
- The debugger attaches at most once per process (to avoid multiple debuggers competing for the pipe).

If you prefer to start it manually in Lua, you can call:

```lua
require("wrkr/debug").start()
```
