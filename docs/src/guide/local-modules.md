# Local modules (vendoring)

`wrkr` prepends the script directory to `package.path`, so scripts can `require()` Lua files shipped next to the script.

It adds these patterns:

- `${SCRIPT_DIR}/?.lua`
- `${SCRIPT_DIR}/?/init.lua`

Example layout:

- `benchmark.lua`
- `lib/checks.lua`

```lua
local checks = require("lib.checks")
```
