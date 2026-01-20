# Running scripts

## Basic

```bash
wrkr run path/to/script.lua
```

Most scripts use environment variables for configuration (like `BASE_URL`).

```bash
BASE_URL="https://example.com" wrkr run examples/plaintext.lua
```

## CLI overrides

CLI flags override values from the script’s global `options` table.

```bash
wrkr run examples/plaintext.lua --vus 50 --duration 30s
wrkr run examples/json_aggregate.lua --iterations 1000
wrkr run examples/plaintext.lua --env BASE_URL=https://example.com
```

## Output formats

- Default: human summary.
- JSON progress lines (NDJSON):

```bash
wrkr run examples/plaintext.lua --output json
```

## Local test server (repo)

If you’re working in this repository, you can run a local test server used by examples:

```bash
cargo run --bin wrkr-testserver
```

It prints a `BASE_URL=...` line when ready.
