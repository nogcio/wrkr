# wrkr-tools-coverage

Small helper to generate + inspect Rust line coverage via `cargo llvm-cov`.

## Usage

Generate lcov (runs tests):

```bash
uv run --project . wrkr-tools-coverage gen --workspace
```

Report lowest-covered files:

```bash
uv run --project . wrkr-tools-coverage report --top 30 --min-lines 25
```

JSON output (for further processing):

```bash
uv run --project . wrkr-tools-coverage report --format json > target/coverage.json
```
