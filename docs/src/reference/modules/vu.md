# wrkr/vu

Virtual-user helpers.

```lua
local vu = require("wrkr/vu")
```

## `vu.id() -> integer`

Returns a stable numeric VU id.

Notes:

- During the options-parsing phase, `vu.id()` is `0`.
