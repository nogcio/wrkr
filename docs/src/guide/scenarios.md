# Scenarios & executors

`wrkr` supports multiple scenarios via `options.scenarios`.

A scenario can override `vus`, `duration`, and `iterations`, and can choose an executor.

## constant-vus (default)

If `executor` is omitted, the scenario runs with a constant number of VUs.

```lua
options = {
  scenarios = {
    main = { executor = "constant-vus", vus = 10, duration = "10s", exec = "Default" },
  },
}
```

## ramping-vus

Ramp the number of active VUs up/down over time.

```lua
options = {
  scenarios = {
    main = {
      executor = "ramping-vus",
      startVUs = 0,
      stages = {
        { duration = "10s", target = 50 },
        { duration = "10s", target = 0 },
      },
      exec = "Default",
    },
  },
}
```

## ramping-arrival-rate

Ramp an open-model arrival rate (iterations started per `timeUnit`), with adaptive VU activation.

```lua
options = {
  scenarios = {
    main = {
      executor = "ramping-arrival-rate",
      startRate = 10,
      timeUnit = "1s",
      preAllocatedVUs = 10,
      maxVUs = 200,
      stages = {
        { duration = "10s", target = 100 },
        { duration = "10s", target = 10 },
      },
      exec = "Default",
    },
  },
}
```

See also: [Executors](../reference/executors.md).
