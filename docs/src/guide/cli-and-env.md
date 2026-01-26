# CLI overrides & env

## Precedence

`wrkr` has two separate “inputs” for a run:

1. **Run configuration** (iterations/vus/duration/scenarios):
	- Script `options` table provides defaults.
	- CLI flags (e.g. `--vus`, `--duration`, `--iterations`) override the script.
2. **Environment variables** (read in Lua via `require("wrkr/env")`):
	- The current process environment is visible to the script.
	- CLI `--env KEY=VALUE` entries override the current process env for that run.

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
