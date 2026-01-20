# wrkr/json

JSON encode/decode helpers.

```lua
local json = require("wrkr/json")
```

## `json.encode(value) -> string`

Encodes a Lua value into JSON.

## `json.decode(string) -> any`

Decodes JSON into Lua values.

## Example

```lua
local json = require("wrkr/json")
local s = json.encode({ hello = "world" })
local v = json.decode(s)
```
