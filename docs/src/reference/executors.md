# Executors

Executors define how a scenario schedules work.

## constant-vus

Runs with a constant number of virtual users.

Scenario fields:

- `vus` (required unless provided at top-level)
- `duration` (optional)
- `iterations` (optional)
- `exec` (optional, defaults to `"Default"`)

## ramping-vus

Ramp VUs over a sequence of stages.

Scenario fields:

- `startVUs` (or `start_vus`, default `0`)
- `stages` (list of `{ duration, target }`)

## ramping-arrival-rate

Open model executor: ramp iterations started per `timeUnit`.

Scenario fields:

- `startRate` (or `start_rate`)
- `timeUnit` (or `time_unit`, duration string)
- `preAllocatedVUs` (or `pre_allocated_vus`)
- `maxVUs` (or `max_vus`)
- `stages` (list of `{ duration, target }`) where target is a rate

See [Scenarios & executors](../guide/scenarios.md) for examples.
