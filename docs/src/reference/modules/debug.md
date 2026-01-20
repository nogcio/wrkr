# wrkr/debug

Helpers for starting a VS Code Lua debugger (best-effort).

```lua
local debug = require("wrkr/debug")
```

## `debug.start() -> boolean`

Attempts to start the debugger.

## `debug.maybe_start() -> nil`

Starts the debugger only if the process environment indicates VS Code debugging.
