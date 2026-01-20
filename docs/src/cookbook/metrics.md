# Custom metrics

Use `wrkr/metrics` to define custom metrics and attach tags.

```lua
local metrics = require("wrkr/metrics")

local ok_rate = metrics.Rate("my_ok")

function Default()
  local ok = true
  ok_rate:add(ok, { scenario = "main" })
end
```

For grouping/tagging, see [wrkr/group](../reference/modules/group.md).
