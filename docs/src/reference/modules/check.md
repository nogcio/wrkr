# wrkr/check

Runs named checks and records pass/fail.

```lua
local check = require("wrkr/check")
```

## `check(value, checks) -> boolean`

- `value`: table
- `checks`: table of `name -> function(value) return boolean end`

Returns `true` if all checks pass, otherwise `false`.

Each check result is recorded as a metric.

## Example

```lua
local http = require("wrkr/http")
local check = require("wrkr/check")
local env = require("wrkr/env")

function Default()
  local res = http.get(env.BASE_URL .. "/plaintext")
  check(res, {
    ["status is 200"] = function(r) return r.status == 200 end,
  })
end
```
