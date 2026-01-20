# wrkr/shared

A shared key/value store and counters for coordinating between VUs.

```lua
local shared = require("wrkr/shared")
local check = require("wrkr/check")
```

## Key/value

### `shared.get(key) -> any|nil`

Returns the value or `nil` if missing.

### `shared.set(key, value) -> nil`

Stores a value.

### `shared.delete(key) -> nil`

Deletes the key (and any counter with the same name).

## Counters

### `shared.incr(key, delta?) -> integer`

Increments by `delta` (default `1`) and returns the new value.

### `shared.counter(key) -> integer`

Returns the counter value (default `0` if missing).

## Coordination (async)

### `shared.wait(key) -> any`

Waits until `key` exists, then returns its value.

### `shared.barrier(name, parties) -> nil`

Waits on a named barrier for `parties` participants.

## Example

```lua
local shared = require("wrkr/shared")

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

Tip: `wrkr/check` expects the first argument to be a Lua table. If you want to check a scalar, wrap it into a table (like `ctx` above).
