# Quick start

## Install

Pick one:

- Homebrew (macOS):

```bash
brew tap nogcio/wrkr
brew install wrkr
```

- GitHub Releases (binaries): download `wrkr` and put it on your `PATH`.

- Docker:

```bash
docker pull ghcr.io/nogcio/wrkr:<version>
```

If you’re developing locally (from source):

```bash
cargo build --release
```

Binary will be at `./target/release/wrkr`.

## 1) Scaffold a script workspace (recommended)

If you want editor autocomplete + type hints (LuaLS), run:

```bash
wrkr init --lang lua
```

This writes:

- `.luarc.json`
- `.wrkr/lua-stubs/`
- `script.lua`

VS Code recommendations (optional):

```bash
wrkr init --lang lua --vscode
```

## 2) Run a script

Run one of the repository examples:

```bash
BASE_URL="https://example.com" wrkr run examples/plaintext.lua
```

Or via `cargo run` while developing:

```bash
BASE_URL="https://example.com" cargo run --bin wrkr -- run examples/plaintext.lua
```

## 3) (Optional) Run the local test server

This repo includes a small HTTP/gRPC test server for examples.
It prints `HTTP_URL=...` and `GRPC_URL=...` lines once ready.

```bash
cargo run --bin wrkr-testserver
```

In another terminal:

```bash
BASE_URL="http://127.0.0.1:12345" wrkr run examples/plaintext.lua
```
