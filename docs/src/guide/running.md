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

CLI flags override values from the script’s global `Options` table.

```bash
wrkr run examples/plaintext.lua --vus 50 --duration 30s
wrkr run examples/json_aggregate.lua --iterations 1000
wrkr run examples/plaintext.lua --env BASE_URL=https://example.com
```

## Selecting a scenario

If your script defines `Options.scenarios`, you can run a single scenario by name:

```bash
wrkr run examples/plaintext.lua --scenario main
```

You can also pass a YAML file describing one or more scenarios. In this mode, `wrkr` does **not**
execute the script to parse `Options` (it only runs the scenario entry function during the run).

```bash
wrkr run examples/plaintext.lua --scenario path/to/scenario.yaml
```

Minimal YAML (flat form):

```yaml
name: main
executor: constant-vus
vus: 10
duration: 10s
exec: Default
```

The export command writes a multi-scenario form (top-level `scenarios:` list), and `wrkr run` can
consume that file directly.

## Exporting a scenario to YAML

To export the resolved scenario configuration(s) (after applying the same CLI overrides as
`wrkr run`) without executing a run:

```bash
wrkr scenario export examples/plaintext.lua --out scenarios.yaml
```

## Output formats

- Default: human summary.
- JSON progress lines (NDJSON):

```bash
wrkr run examples/plaintext.lua --output json
```

`--output json` prints one JSON object per line (NDJSON):

- Every line includes `schema: "wrkr.ndjson.v1"` and a `kind` discriminator.
- `kind: "progress"` lines are emitted periodically during the run.
- A final `kind: "summary"` line is emitted at the end.
- JSON keys are camelCase; time/latency values are seconds as floats (e.g. `elapsedSeconds`, `intervalSeconds`, `latencySeconds`).
- The final summary line includes `thresholds.violations` for machine-readable quality-gate results.

JSON Schema:

- https://github.com/nogcio/wrkr/blob/main/schemas/wrkr.ndjson.v1.line.schema.json

## Local test server (repo)

If you’re working in this repository, you can run a local test server used by examples:

```bash
cargo run --bin wrkr-testserver
```

It prints `HTTP_URL=...` and `GRPC_URL=...` lines when ready.
