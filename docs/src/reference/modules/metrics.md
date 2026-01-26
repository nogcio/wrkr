# wrkr/metrics

Create and record custom metrics.

```lua
local metrics = require("wrkr/metrics")
```

## Metric tags

Most metric operations accept optional tags:

```lua
{ route = "/plaintext", status = 200, ok = true }
```

If you call a metric inside a [wrkr/group](group.md) group, a `group` tag is added unless you already set one.

## Constructors

### `metrics.Trend(name) -> metric`
### `metrics.Counter(name) -> metric`
### `metrics.Gauge(name) -> metric`
### `metrics.Rate(name) -> metric`

`name` must be non-empty.

Note: metric names are global within a run. Avoid using names reserved by `wrkr` itself (for example `vu_active` / `vu_active_max`). Reusing a built-in name will mix your updates into internal metrics and can produce confusing summaries (including negative `vu_active`).

## `metric:add(value, tags?)`

- Trend/Counter/Gauge: `value` is a number
- Rate: `value` is a boolean

## Example

```lua
local metrics = require("wrkr/metrics")
local http = require("wrkr/http")
local env = require("wrkr/env")

local latency = metrics.Trend("my_latency")

function Default()
  local started = os.clock()
  local res = http.get(env.BASE_URL .. "/plaintext")
  local elapsed_us = (os.clock() - started) * 1000000
  latency:add(elapsed_us, { status = res.status })
end
```
