# wrkr/group

Run work under a named group.

Groups are typically used for tagging metrics (HTTP and custom metrics).

```lua
local group = require("wrkr/group")
```

## `group.group(name, f) -> any`

- `name`: string
- `f`: function (can be async)

Runs `f()` while the current group is set to `name`, then restores the previous group.

## Example

```lua
local group = require("wrkr/group")
local http = require("wrkr/http")
local env = require("wrkr/env")

function Default()
  group.group("home", function()
    http.get(env.BASE_URL .. "/")
  end)
end
```
