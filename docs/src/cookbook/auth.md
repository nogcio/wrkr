# Auth & headers

Most scripts pass auth via headers.

```lua
local http = require("wrkr/http")
local check = require("wrkr/check")
local env = require("wrkr/env")

function Default()
  local res = http.get(env.BASE_URL .. "/plaintext", {
    headers = {
      authorization = "Bearer " .. env.TOKEN,
    },
    name = "GET /plaintext",
  })
  check(res, {
    ["status is 200"] = function(r) return r.status == 200 end,
    ["no transport error"] = function(r) return r.error == nil end,
  })
end
```

Tips:

- Prefer `Setup()` + `wrkr/shared` if you need to fetch a token once and reuse it.
