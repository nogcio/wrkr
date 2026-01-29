# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/)
(and this file is parsed by the release workflow).

## [Unreleased]

### Added



### Changed



### Fixed



## [0.1.2] - 2026-01-30

### Added
- CI/Docker: add a manual publish path (`workflow_dispatch`) to (re)publish container images for a specific git tag.
- CI/Docker: add a smoke test to ensure the built image reports the expected `wrkr --version`.

### Changed
- Docker: publish release images to GitHub Container Registry (GHCR) (`ghcr.io/nogcio/wrkr`).

### Fixed
- Docker: prevent publishing/tagging a stale `wrkr` binary under newer release tags.

## [0.1.1] - 2026-01-27

### Added
- CLI: `--scenario PATH.yml/.yaml` mode to load scenarios from YAML and skip parsing script `Options`.
- CLI: `wrkr scenario export` to export resolved scenario configuration to YAML without executing a run.

### Changed
- Scenario YAML supports multi-scenario files (`scenarios: [...]`) as exported by `wrkr scenario export` and consumable by `wrkr run`.

## [0.1.0] - 2026-01-27

### Changed
- **Breaking**: rename `GRPC_TARGET` to `BASE_URL` across scripts/config (gRPC and HTTP examples now share the same env var).

## [0.0.9] - 2026-01-27

### Added
- HTTP: support `https://` URLs for HTTP checks (TLS via rustls).

### Changed
- HTTP: replace the `OnlyHttpSupported` error with `UnsupportedScheme` (now allows both `http://` and `https://`).

## [0.0.8] - 2026-01-27

### Added
- Output: publish the NDJSON v1 JSON output contract (docs + JSON Schemas under `schemas/`).
- Output: include thresholds results in the NDJSON v1 summary line (`thresholds.violations`).
- Thresholds: support tag selectors in metric keys (e.g. `http_req_duration{group=login,method=GET}`) and document the selector rules.
- Lua HTTP: add verb helpers (`put`, `patch`, `delete`, `head`, `options`) and tests.
- Examples: add a thresholds grouping example (`examples/thresholds_group.lua`).

### Changed
- Core: compute summary metrics and evaluate thresholds in `wrkr-core`, including tag-selector matching over metric series (fixes #19).
- Output/Tools: refine JSON (NDJSON) output and update the perf harness parser accordingly.
- CLI: standardize `wrkr run` failure classification via stable exit codes (checks/thresholds/script/invalid input/runtime) (fixes #20).
- Lua HTTP: always record the HTTP method as a stable request metric tag (`method`), overriding any user-supplied `method` tag.

### Fixed
- Lua checks: ensure `wrkr/check` accepts any value (regression test).

## [0.0.7] - 2026-01-26

### Added
- HTTP: support additional methods (`PUT`, `PATCH`, `DELETE`, `HEAD`, `OPTIONS`) and a custom-method escape hatch (`http.request`).
- gRPC: add wire encode/decode tests.
- Tools: migrate perf comparison tooling to Python (`tools/compare-perf`) and add a coverage helper (`tools/coverage`).

### Changed
- Metrics: record and report latencies in microseconds (instead of milliseconds), including docs updates.
- Output: improve human summary formatting and JSON (NDJSON) progress/summary output; update perf parser accordingly.
- Core: refactor runner/metrics internals and protocol module boundaries (HTTP/gRPC extraction and cleanup).
- Lua: update LuaLS stubs and module docs.

### Fixed
- Tools: fix summary parsing and compare logic in the perf harness.

## [0.0.6] - 2026-01-23

### Fixed
- Checks: remove automatic failure recording for HTTP status codes >= 400 to allow explicit check handling.

## [0.0.5] - 2026-01-23

### Added
- gRPC: add a message encoder so unary requests can be pre-encoded (Lua: `grpc.Client:encode`).

### Changed
- Tools: update the WFB gRPC aggregation perf script to use pre-encoded request bytes.

## [0.0.4] - 2026-01-23

### Changed
- gRPC: improve request/response encoding/decoding performance by using a bytes-based gRPC codec and a custom protobuf wire encoder/decoder for `wrkr_value::Value`.
- Tools: extend `wrkr-tools-compare-perf` with a WFB gRPC aggregation case and an optional cross-protocol ratio check.

## [0.0.3] - 2026-01-23

### Added
- Tools: add `wrkr-tools-compare-perf` harness for comparing `wrkr` performance against `wrk` and `k6`.
- gRPC: support connection pooling via a shared client and expose `pool_size` in `wrkr.grpc.Client.new` options.

### Changed
- Tools: remove legacy perf comparison script (`tools/perf/compare_wrk.sh`) in favor of the Rust harness and dedicated tool scripts.
- gRPC: reduce metrics hot-path contention by caching tagged series and reorganizing latency tracking.
- `wrkr-value`: switch to `ahash` for faster hash maps.

### Fixed
- gRPC: record response message bytes (encoded protobuf) rather than transport bytes.
- Lua gRPC: validate `pool_size` is a finite positive integer and within reasonable bounds.
- Tools: improve diagnostics output size for perf parser failures.

## [0.0.2] - 2026-01-21

### Added
- CI: nextest configuration and CI-profile test execution with JUnit output.

### Changed
- Docs: expanded maintainer/release guidance and issue/label conventions.

### Fixed
- CI: publish JUnit test reports via `mikepenz/action-junit-report`.
- CI: allow reusable checks workflow to request `checks: write` and `pull-requests: read`.
- CI: use system LuaJIT and `protoc` where appropriate.
- CI: fix Windows `protoc` include discovery for bundling.
- CI: fix Windows LuaJIT build by initializing MSVC environment.
- CI: fix Windows LuaJIT linking env export and verify `lua51.lib` is present.
- CI: fix Windows LuaJIT build step to `call msvcbuild.bat` so copy steps run.
- CI: fix Windows linking by selecting MSVC `link.exe` (avoid Git `link.exe`).
- CI: fix Homebrew formula update script quoting.
- CI: fix macOS x86_64 build by using an Intel runner (avoid cross `pkg-config`/LuaJIT discovery issues).

## [0.0.1] - 2026-01-20

### Added
- Lua scripting engine (`wrkr-lua`) with a small, module-based API (`require("wrkr/...")`).
- Script contract: `options`, entrypoint `Default()`, and lifecycle hooks `Setup()`, `Teardown()`, `HandleSummary(summary)`.
- Executors: `constant-vus`, `ramping-vus`, `ramping-arrival-rate`.
- CLI overrides for runs: `--vus`, `--duration`, `--iterations`, `--env KEY=VALUE`, `--output`.
- Output formats: human summary and JSON progress lines (NDJSON).
- Built-in Lua modules: `wrkr/http`, `wrkr/check`, `wrkr/env`, `wrkr/json`, `wrkr/vu`, `wrkr/shared`, `wrkr/fs` (+ aggregate `wrkr`).
- `wrkr init --lang lua` to scaffold a script workspace with `.luarc.json` and LuaLS stubs.
- Example scripts under `examples/` (HTTP, JSON aggregation, gRPC aggregation, lifecycle hooks).
- Distribution: GitHub Release binaries, Docker image on release tags, and Homebrew formula.

[Unreleased]: https://github.com/nogcio/wrkr/compare/v0.1.2...HEAD
[0.1.2]: https://github.com/nogcio/wrkr/compare/v0.1.1...v0.1.2
[0.1.1]: https://github.com/nogcio/wrkr/compare/v0.1.0...v0.1.1
[0.1.0]: https://github.com/nogcio/wrkr/compare/v0.0.9...v0.1.0
[0.0.9]: https://github.com/nogcio/wrkr/compare/v0.0.8...v0.0.9
[0.0.8]: https://github.com/nogcio/wrkr/compare/v0.0.7...v0.0.8
[0.0.7]: https://github.com/nogcio/wrkr/compare/v0.0.6...v0.0.7
[0.0.6]: https://github.com/nogcio/wrkr/compare/v0.0.5...v0.0.6
[0.0.5]: https://github.com/nogcio/wrkr/compare/v0.0.4...v0.0.5
[0.0.4]: https://github.com/nogcio/wrkr/compare/v0.0.3...v0.0.4
[0.0.3]: https://github.com/nogcio/wrkr/compare/v0.0.2...v0.0.3
[0.0.2]: https://github.com/nogcio/wrkr/compare/v0.0.1...v0.0.2
[0.0.1]: https://github.com/nogcio/wrkr/releases/tag/v0.0.1
