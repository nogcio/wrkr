# plaintext.lua

Basic HTTP GET + checks.

Source:

- [examples/plaintext.lua on GitHub](https://github.com/nogcio/wrkr/blob/main/examples/plaintext.lua)

## Run

### Option A: using the repo test server

Start the server:

```bash
cargo run --bin wrkr-testserver
```

In another terminal (use the printed `BASE_URL=...`):

```bash
BASE_URL="http://127.0.0.1:12345" wrkr run examples/plaintext.lua
```

### Option B: against your own target

```bash
BASE_URL="https://example.com" wrkr run examples/plaintext.lua
```

## What it shows

- `wrkr/http.get`
- `wrkr/check`
- Using `wrkr/env` (`BASE_URL`)
- Vendored helpers (`require("lib.checks")`)
