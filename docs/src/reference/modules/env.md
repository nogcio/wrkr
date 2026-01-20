# wrkr/env

A table of environment variables visible to the script.

```lua
local env = require("wrkr/env")
print(env.BASE_URL)
```

Notes:

- Values are strings.
- CLI `--env KEY=VALUE` overrides the current process env for that run.
