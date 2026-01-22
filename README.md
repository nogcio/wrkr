# wrkr

Fast load-testing tool written in Rust.

Today, `wrkr` is script-driven via Lua (see [`wrkr-lua/README.md`](wrkr-lua/README.md)), but the project is intentionally structured around a small, explicit scripting API so additional scripting approaches can be added over time.

## Features

- Rust performance, low overhead
- Script-driven scenarios and checks (current engine: Lua)
- Scenarios/executors: `constant-vus`, `ramping-vus`, `ramping-arrival-rate`
- Per-run overrides via CLI flags (`--vus`, `--duration`, `--iterations`, `--env KEY=VALUE`)
- Human summary output or JSON progress lines (NDJSON) via `--output`
- Live dashboard server (HTML + WebSocket) via `--dashboard`

## Install

### Homebrew (macOS)

Install via a formula URL (no Rust toolchain required):

```bash
brew install --formula https://raw.githubusercontent.com/nogcio/wrkr/main/homebrew/wrkr.rb
```

Note: the formula in `main` is updated by CI when you publish a release tag like `vX.Y.Z`.

### GitHub Releases (binaries)

Download a prebuilt binary from GitHub Releases and put `wrkr` on your `PATH`.

### Docker

Pull the published image (built on release tags):

```bash
docker pull nogcio/wrkr:<version>
```

### From source (development)

This project is distributed via GitHub Releases (binaries), Docker, and Homebrew.
If you’re developing locally:

```bash
cargo build --release
```

Binary will be at `./target/release/wrkr`.

## Quick start

### 0) Create a script workspace (recommended)

If you want editor autocomplete + type hints for `require("wrkr/...")` modules (LuaLS), scaffold a small workspace:

```bash
wrkr init
```

This writes:

- `.luarc.json` configured to use bundled LuaLS stubs
- `.wrkr/lua-stubs/` (LuaLS type stubs)
- `script.lua` (a small starter script)

Optional (VS Code):

```bash
wrkr init --vscode
```

### 1) Run the local test server (optional)

This repo includes a tiny HTTP server that exposes endpoints used by the example scripts.
It prints a `BASE_URL=...` line once it is ready.

```bash
cargo run --bin wrkr-testserver
```

In another terminal:

```bash
BASE_URL="http://127.0.0.1:12345" ./target/release/wrkr run examples/plaintext.lua
```

### 2) Run against any target

```bash
BASE_URL="https://example.com" ./target/release/wrkr run examples/plaintext.lua
```

Or via `cargo run`:

```bash
BASE_URL="https://example.com" cargo run --bin wrkr -- run examples/plaintext.lua
```

## Usage

```bash
wrkr run <script.lua> [--vus N] [--duration 10s] [--iterations N] [--env KEY=VALUE] [--output human-readable|json] [--dashboard] [--dashboard-port <port> | --dashboard-bind <addr>]
```

Notes:

- CLI flags override values from the script's global `options` table.
- Environment variables from the current process are visible to the script; use `--env KEY=VALUE` to add/override values for a single run.

### Dashboard (local)

Enable a local live dashboard server (table per scenario) and stream progress updates via WebSocket:

```bash
wrkr run examples/plaintext.lua --dashboard
```

`wrkr` prints a `dashboard=http://127.0.0.1:<port>` line to stderr; open it in your browser.

Notes:

- You can also enable the dashboard via env: `WRKR_DASHBOARD=1` (equivalent to `--dashboard`).
- The server binds to loopback only (e.g. `127.0.0.1`). Remote binding is intentionally rejected.
- You can configure the bind via `--dashboard-bind` (use `:0` for an ephemeral port) or `--dashboard-port`.

## Development rules (Cursor)

This repository has explicit development rules for Cursor:

- Canonical: `.github/copilot-instructions.md`
- Cursor: `.cursorrules`

Examples:

```bash
wrkr run examples/plaintext.lua --vus 50 --duration 30s
wrkr run examples/json_aggregate.lua --iterations 1000 --output json
wrkr run examples/grpc_aggregate.lua --env GRPC_TARGET=http://127.0.0.1:50051
wrkr run examples/plaintext.lua --env BASE_URL=https://example.com
```

## Scripting (Lua today)

At the moment, scripts are Lua files that typically:

- Define an optional global `options` table (defaults + scenarios).
- Export an entry function `Default()` (and optionally more functions referenced by scenarios via `exec`).
- Optionally define lifecycle hooks `Setup()`, `Teardown()`, and `HandleSummary(summary)`.

Built-in APIs are accessed via modules (no globals):

```lua
local http = require("wrkr/http")
local check = require("wrkr/check")
```

Scripts can also vendor local Lua modules next to the script (the runner prepends the script directory to `package.path`).

Full details (script contract, executors, and module reference): [`wrkr-lua/README.md`](wrkr-lua/README.md)

## Examples

See [`examples/`](examples/) for ready-to-run scripts:

- `examples/plaintext.lua` (basic GET + checks)
- `examples/json_aggregate.lua` (JSON + aggregation)
- `examples/grpc_aggregate.lua` (gRPC + aggregation)
- `examples/lifecycle.lua` (Setup/Teardown/HandleSummary hooks)
- `examples/ramping_vus.lua`
- `examples/ramping_arrival_rate.lua`

## Documentation

This repo includes an mdBook under `docs/`.

- Build: `mdbook build docs`
- Serve locally: `mdbook serve docs --open`

Published docs (GitHub Pages): `https://nogcio.github.io/wrkr/`

## Development

Common commands:

```bash
cargo fmt --all
cargo clippy --all-targets -- -D warnings
cargo test --workspace
```

Or via `make`:

```bash
make check
make run SCRIPT=examples/plaintext.lua BASE_URL=https://example.com WRKR_RUN_ARGS='--vus 50 --duration 10s'
```

## Contributing

- See [`CONTRIBUTING.md`](CONTRIBUTING.md)
- Code of conduct: [`CODE_OF_CONDUCT.md`](CODE_OF_CONDUCT.md)

## Security

See [`.github/SECURITY.md`](.github/SECURITY.md).

## License

GNU Affero General Public License v3.0 (see [`LICENSE`](LICENSE)).
