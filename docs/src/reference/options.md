# Options

A script can define a global `options` table.

```lua
options = {
  vus = 10,
  duration = "10s",
  iterations = 100,
}
```

## Common fields

- `vus` (number, > 0)
- `duration` (string like `"250ms"`, `"10s"`, `"1m"` or a positive number of seconds)
- `iterations` (number, > 0)

## Scenarios

To define multiple scenarios:

```lua
options = {
  scenarios = {
    main = { executor = "constant-vus", vus = 10, duration = "10s", exec = "Default" },
  }
}
```

Scenario fields support both snake_case and camelCase for some keys:

- `startVUs` or `start_vus`
- `startRate` or `start_rate`
- `timeUnit` or `time_unit`
- `preAllocatedVUs` or `pre_allocated_vus`
- `maxVUs` or `max_vus`

See [Executors](executors.md) for executor-specific fields.

## Thresholds

`wrkr` can evaluate thresholds at the end of a run.

```lua
options = {
  thresholds = {
    http_req_duration = { "p(95) < 200" },
    http_req_failed = "rate < 0.01",
  },
}
```

See [Thresholds](thresholds.md).
