# Debugging in VS Code

`wrkr` can start a Lua debugger (best-effort) when it detects VS Code debugging.

Recommended extensions:

- `sumneko.lua`
- `tomblind.local-lua-debugger-vscode`

You can also start it manually from Lua:

```lua
require("wrkr/debug").start()
```

Or let `wrkr` decide based on environment:

```lua
require("wrkr/debug").maybe_start()
```

Notes:

- Debugging mode may use `mlua::Lua::unsafe_new()` internally to expose the `debug` stdlib for `lldebugger`.
- The debugger attaches at most once per process.
