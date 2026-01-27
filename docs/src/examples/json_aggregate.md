# json_aggregate.lua

HTTP POST JSON workload with validation.

Source:

- [examples/json_aggregate.lua on GitHub](https://github.com/nogcio/wrkr/blob/main/examples/json_aggregate.lua)

## Run

### Option A: using the repo test server

Start the server:

```bash
cargo run --bin wrkr-testserver
```

In another terminal (use the printed `HTTP_URL=...`):

```bash
BASE_URL="http://127.0.0.1:12345" wrkr run examples/json_aggregate.lua
```

### Option B: against your own target

This script expects an endpoint:

- `POST /json/aggregate`

So you need a compatible backend.

```bash
BASE_URL="https://example.com" wrkr run examples/json_aggregate.lua
```

## What it shows

- `http.post` with automatic JSON encoding
- Validating complex responses via helper checks
- Reusing generated payloads via a pool (`lib.pool`)
