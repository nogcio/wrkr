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

## Tag selectors

Threshold keys may include an optional tag selector block:

- Base (all series aggregated): `http_req_duration`
- Tag-scoped: `http_req_duration{group=login}`
- Multiple tags: `http_req_duration{group=login,method=GET}`

Rules (v1):

- Whitespace inside `{ ... }` is ignored.
- Tag order does not matter.
- Keys/values are “simple” strings (no escaping/quoting).

The selector matches series that contain **all** the specified tags; extra tags on the series do not prevent a match.

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

- Thresholds without a selector are evaluated over the global aggregate for the metric.
- If no matching series exists for a selector, the threshold fails.
