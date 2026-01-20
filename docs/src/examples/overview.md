# Examples

The repository includes ready-to-run scripts under:

- [examples/ on GitHub](https://github.com/nogcio/wrkr/tree/main/examples)

This section explains what each script does and how to run it.

## Index

| Example | What it demonstrates |
| --- | --- |
| [plaintext.lua](plaintext.md) | Basic HTTP GET + checks |
| [json_aggregate.lua](json_aggregate.md) | JSON POST workload + validation |
| [grpc_aggregate.lua](grpc_aggregate.md) | gRPC unary client + checks |
| [lifecycle.lua](lifecycle.md) | Setup/Teardown/HandleSummary |
| [ramping_vus.lua](ramping_vus.md) | VU ramping stages |
| [ramping_arrival_rate.lua](ramping_arrival_rate.md) | Open-model arrival-rate ramp |

Common patterns:

- Most scripts read `BASE_URL` from `wrkr/env`.
- gRPC scripts usually require `GRPC_TARGET`.

Tip: the repo also contains a local test server used by the examples:

```bash
cargo run --bin wrkr-testserver
```

Then run examples using the printed `BASE_URL=...`.
