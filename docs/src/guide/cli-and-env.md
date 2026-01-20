# CLI overrides & env

## Precedence

At runtime, values typically come from:

1. Script `options` table (defaults)
2. CLI flags (override script)
3. Environment variables (available to the script via `require("wrkr/env")`)

CLI flags always override script options.

## Env vars

All current process env vars are visible to the script.

To add/override env vars for a single run:

```bash
wrkr run examples/plaintext.lua --env BASE_URL=https://example.com
wrkr run examples/grpc_aggregate.lua --env GRPC_TARGET=http://127.0.0.1:50051
```

In Lua:

```lua
local env = require("wrkr/env")
print(env.BASE_URL)
```
