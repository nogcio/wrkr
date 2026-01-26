# wrkr/http

HTTP client helpers.

```lua
local http = require("wrkr/http")
```

## Functions

### `http.get(url, opts?) -> res`

- `url`: string
- `opts` (optional table):
  - `headers`: table of header name → value
  - `params`: table of query param name → value
  - `timeout`: number (seconds) or duration string (e.g. `"250ms"`, `"10s"`)
  - `tags`: table<string, string|number|boolean>
  - `name`: string (request metric tag `name` override)

Returns a table:

- `status`: integer (`0` on transport error)
- `body`: string
- `headers`: table<string, string> (lowercased header names)
- `error`: string? (present on transport error)

### `http.post(url, body, opts?) -> res`

Same options/return shape as `get`.

Body handling:

- If `body` is a Lua string, it’s sent as-is with default content-type `text/plain; charset=utf-8`.
- Otherwise, `body` is JSON-encoded (using `wrkr/json`) and default content-type is `application/json; charset=utf-8`.

If `opts.headers` already contains `Content-Type`, it is not overridden.

### `http.put(url, body, opts?) -> res`

Same options/return shape as `post`.

### `http.patch(url, body, opts?) -> res`

Same options/return shape as `post`.

### `http.delete(url, opts?) -> res`

Same options/return shape as `get`.

### `http.head(url, opts?) -> res`

Same options/return shape as `get`.

### `http.options(url, opts?) -> res`

Same options/return shape as `get`.

### `http.request(method, url, body?, opts?) -> res`

Custom method escape hatch.

## Example

```lua
local http = require("wrkr/http")
local check = require("wrkr/check")
local env = require("wrkr/env")

local res = http.get(env.BASE_URL .. "/plaintext", { name = "GET /plaintext" })
check(res, {
  ["status is 200"] = function(r) return r.status == 200 end,
  ["no transport error"] = function(r) return r.error == nil end,
})
```
