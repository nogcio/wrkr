# grpc_aggregate.lua

gRPC unary workload based on `examples/protos/analytics.proto`.

Source:

- [examples/grpc_aggregate.lua on GitHub](https://github.com/nogcio/wrkr/blob/main/examples/grpc_aggregate.lua)

## Required env

- `BASE_URL` (e.g. `127.0.0.1:50051` or `http://127.0.0.1:50051`)

## Run

### Option A: using the repo test server

Start the gRPC test server:

```bash
cargo run --bin wrkr-testserver
```

In another terminal:

```bash
# Use the value printed as GRPC_URL=...
wrkr run examples/grpc_aggregate.lua --env BASE_URL=127.0.0.1:50051
```

### Option B: against your own target

```bash
wrkr run examples/grpc_aggregate.lua --env BASE_URL=127.0.0.1:50051
```

## What it shows

- `wrkr/grpc` client lifecycle: `load` → `connect` → `invoke`
- Per-request metadata and tags
- Reusing generated payloads via a pool
