# Lua API overview

`wrkr` scripts access built-in functionality via Lua modules (no globals).

```lua
local http = require("wrkr/http")
local check = require("wrkr/check")
local env = require("wrkr/env")
```

There is also a convenience aggregate module:

```lua
local wrkr = require("wrkr")
wrkr.http.get(...)
```

## Runtime modules

- HTTP: `wrkr/http`
- gRPC: `wrkr/grpc`
- Checks: `wrkr/check`
- Env: `wrkr/env`
- JSON: `wrkr/json`
- File reads: `wrkr/fs`
- Shared store: `wrkr/shared`
- VU info: `wrkr/vu`
- Grouping: `wrkr/group`
- Custom metrics: `wrkr/metrics`
- UUIDs: `wrkr/uuid`
- Debugger helpers: `wrkr/debug`

See [Modules](modules.md).
