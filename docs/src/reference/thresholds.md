# Thresholds

Thresholds are end-of-test assertions over aggregated metrics.

In Lua, thresholds live under `options.thresholds` and map metric name → expression(s).

```lua
options = {
  thresholds = {
    http_req_duration = { "p(95) < 200", "avg < 50" },
    http_req_failed = "rate < 0.01",
  },
}
```

## Expression format

An expression is:

- aggregation: `avg`, `min`, `max`, `count`, `rate`, or `p(N)` where `1 <= N <= 100`
- operator: `<`, `<=`, `>`, `>=`, `==`
- numeric value

Whitespace is ignored.

Examples:

- `p(95) < 200`
- `avg <= 50`
- `rate < 0.01`
- `count > 1000`

Notes:

- Thresholds are evaluated on *untagged* series (global totals for the metric).
- If a metric is missing, the threshold fails.
