# JSON requests

`wrkr/http.post` automatically JSON-encodes non-string bodies.

```lua
local http = require("wrkr/http")
local check = require("wrkr/check")
local env = require("wrkr/env")

function Default()
  local res = http.post(env.BASE_URL .. "/json", { hello = "world" })
  check(res, {
    ["status is 200"] = function(r) return r.status == 200 end,
    ["no transport error"] = function(r) return r.error == nil end,
  })
end
```

If you pass a Lua string, it is sent as-is:

```lua
http.post(env.BASE_URL .. "/raw", "hello")
```
