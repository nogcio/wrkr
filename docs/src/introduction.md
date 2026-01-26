# Introduction

`wrkr` is a fast, scriptable load-testing tool written in Rust.

Today the scripting runtime is Lua. The core design goal is to keep a small, explicit scripting API exposed through modules (e.g. `require("wrkr/http")`) so other scripting runtimes can be added later.

## Exit codes

`wrkr run` uses stable, machine-readable exit codes so CI can distinguish quality-gate failures from script/config/runtime errors:

- `0` — success
- `10` — checks failed
- `11` — thresholds failed
- `12` — checks + thresholds failed
- `20` — script error (runtime raised error while executing user script)
- `30` — invalid CLI/config/options (bad flags, invalid durations, invalid thresholds syntax, etc.)
- `40` — internal/runtime error (IO errors, unexpected invariants)

## Where to start

- If you want to run something in 5 minutes: read [Quick start](guide/quickstart.md).
- If you are writing scripts: read [Script contract](guide/script-contract.md) and [Lua API overview](reference/lua-api.md).
- If you want the full surface area: browse [Modules](reference/modules.md) and [Options](reference/options.md).
