# Introduction

`wrkr` is a fast, scriptable load-testing tool written in Rust.

Today the scripting runtime is Lua. The core design goal is to keep a small, explicit scripting API exposed through modules (e.g. `require("wrkr/http")`) so other scripting runtimes can be added later.

## Where to start

- If you want to run something in 5 minutes: read [Quick start](guide/quickstart.md).
- If you are writing scripts: read [Script contract](guide/script-contract.md) and [Lua API overview](reference/lua-api.md).
- If you want the full surface area: browse [Modules](reference/modules.md) and [Options](reference/options.md).
