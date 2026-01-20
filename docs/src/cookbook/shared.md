# Shared store patterns

Use `wrkr/shared` to coordinate between VUs.

## Setup token once

```lua
local shared = require("wrkr/shared")
local check = require("wrkr/check")

function Setup()
  shared.set("token", "abc")
end

function Default()
  local ctx = { token = shared.get("token") }
  check(ctx, {
    ["token exists"] = function(c) return c.token ~= nil end,
  })
end
```

## Counters

```lua
local shared = require("wrkr/shared")

function Default()
  local n = shared.incr("requests")
  if n % 1000 == 0 then
    -- do something occasionally
  end
end
```

## Barriers

```lua
local shared = require("wrkr/shared")
local vu = require("wrkr/vu")

function Default()
  -- wait until all 10 VUs reach this point
  shared.barrier("warmup", 10)
end
```
