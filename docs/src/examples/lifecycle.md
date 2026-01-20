# lifecycle.lua

Shows `Setup`, `Teardown`, and `HandleSummary`.

Source:

- [examples/lifecycle.lua on GitHub](https://github.com/nogcio/wrkr/blob/main/examples/lifecycle.lua)

## Run

### Option A: using the repo test server

Start the server:

```bash
cargo run --bin wrkr-testserver
```

In another terminal:

```bash
BASE_URL="http://127.0.0.1:12345" wrkr run examples/lifecycle.lua
```

### Option B: against your own target

```bash
BASE_URL="https://example.com" wrkr run examples/lifecycle.lua
```

## What it shows

- Using `wrkr/shared` to pass data from `Setup` to VUs
- Adding a custom output file from `HandleSummary`
