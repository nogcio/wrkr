# Script contract

A `wrkr` script is a Lua file with a small, explicit interface.

## Entrypoint

A script must export a function named `Default()`.

```lua
function Default()
  -- executed per VU iteration
end
```

If you define multiple scenarios, each scenario can set `exec` to the function name to run.

## Global `options`

A script may define a global `options` table with defaults:

- `options.vus`
- `options.duration`
- `options.iterations`
- `options.scenarios`
- `options.thresholds`

See [Options](../reference/options.md) for full details.

## Lifecycle hooks (optional)

You can also define these optional hooks:

- `Setup()` — runs once before scenarios (best-effort)
- `Teardown()` — runs once after scenarios (best-effort)
- `HandleSummary(summary)` — runs once after `Teardown()` and can emit extra output/files

```lua
function Setup() end
function Teardown() end

function HandleSummary(summary)
  return {
    stdout = "custom\n",
    stderr = "warnings\n",
    ["summary.json"] = require("wrkr/json").encode(summary) .. "\n",
  }
end
```

Notes:

- Output files returned by `HandleSummary` are written relative to the current working directory.
- `stdout`/`stderr` outputs are printed only when `--output human-readable` is selected (files are still written in all output modes).
- `summary` is a plain Lua table with aggregated totals plus a per-scenario breakdown:
  - Totals: `requests_total`, `failed_requests_total`, `bytes_received_total`, `bytes_sent_total`, `iterations_total`, `checks_failed_total`.
  - Checks: `checks_failed` (table of check name -> count).
  - Per scenario: `scenarios` (array of tables with the same fields plus `scenario`, `checks_failed`, and optional `latency`).
- During the options-parsing phase, `vu.id()` is `0`.
