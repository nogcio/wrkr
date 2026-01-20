# wrkr/fs

Read UTF-8 text files relative to the script path.

```lua
local fs = require("wrkr/fs")
local text = fs.read_file("data/payload.json")
```

## `fs.read_file(rel) -> string`

- `rel`: relative path (from the script file’s directory)

Returns file contents as a string.

Notes:

- This is intended for small input files (payloads, fixtures).
