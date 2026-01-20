# ramping_vus.lua

Ramps the number of virtual users over time.

Source:

- [examples/ramping_vus.lua on GitHub](https://github.com/nogcio/wrkr/blob/main/examples/ramping_vus.lua)

## Run

### Option A: using the repo test server

Start the server:

```bash
cargo run --bin wrkr-testserver
```

In another terminal:

```bash
BASE_URL="http://127.0.0.1:12345" wrkr run examples/ramping_vus.lua
```

### Option B: against your own target

```bash
BASE_URL="https://example.com" wrkr run examples/ramping_vus.lua
```

## What it shows

- `options.scenarios` with `executor = "ramping-vus"`
- `stages = { { duration, target }, ... }`
