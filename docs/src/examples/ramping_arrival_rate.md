# ramping_arrival_rate.lua

Open-model arrival rate executor.

Source:

- [examples/ramping_arrival_rate.lua on GitHub](https://github.com/nogcio/wrkr/blob/main/examples/ramping_arrival_rate.lua)

## Run

### Option A: using the repo test server

Start the server:

```bash
cargo run --bin wrkr-testserver
```

In another terminal:

```bash
BASE_URL="http://127.0.0.1:12345" wrkr run examples/ramping_arrival_rate.lua
```

### Option B: against your own target

```bash
BASE_URL="https://example.com" wrkr run examples/ramping_arrival_rate.lua
```

## What it shows

- `executor = "ramping-arrival-rate"`
- Rate configuration: `startRate`, `timeUnit`, `preAllocatedVUs`, `maxVUs`
