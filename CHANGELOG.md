# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/)
(and this file is parsed by the release workflow).

## [Unreleased]

### Added

### Changed

### Fixed

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
- Tools: add `wrkr-tools-compare-perf` harness for comparing `wrkr` performance against `wrk` and `k6`. ([6f4bb83](https://github.com/nogcio/wrkr/commit/6f4bb832c7eacc9c84979d3226f4bc0c89f3ce96))
- gRPC: support connection pooling via a shared client and expose `pool_size` in `wrkr.grpc.Client.new` options. ([8b09ba5](https://github.com/nogcio/wrkr/commit/8b09ba5692c5b07b49e11f24b5867aaff9440ffe))

### Changed
- Tools: remove legacy perf comparison script (`tools/perf/compare_wrk.sh`) in favor of the Rust harness and dedicated tool scripts. ([6f4bb83](https://github.com/nogcio/wrkr/commit/6f4bb832c7eacc9c84979d3226f4bc0c89f3ce96))
- gRPC: reduce metrics hot-path contention by caching tagged series and reorganizing latency tracking. ([8b09ba5](https://github.com/nogcio/wrkr/commit/8b09ba5692c5b07b49e11f24b5867aaff9440ffe), [fddea04](https://github.com/nogcio/wrkr/commit/fddea040b88f0961dfd4350a4ecf55add70e5be2))
- `wrkr-value`: switch to `ahash` for faster hash maps. ([8b09ba5](https://github.com/nogcio/wrkr/commit/8b09ba5692c5b07b49e11f24b5867aaff9440ffe))

### Fixed
- gRPC: record response message bytes (encoded protobuf) rather than transport bytes. ([fddea04](https://github.com/nogcio/wrkr/commit/fddea040b88f0961dfd4350a4ecf55add70e5be2))
- Lua gRPC: validate `pool_size` is a finite positive integer and within reasonable bounds. ([fddea04](https://github.com/nogcio/wrkr/commit/fddea040b88f0961dfd4350a4ecf55add70e5be2))
- Tools: improve diagnostics output size for perf parser failures. ([fddea04](https://github.com/nogcio/wrkr/commit/fddea040b88f0961dfd4350a4ecf55add70e5be2))

## [0.0.2] - 2026-01-21

### Added
- CI: nextest configuration and CI-profile test execution with JUnit output. ([dccf124](https://github.com/nogcio/wrkr/commit/dccf124b9b6d41b7307011f17fa2e604715678b0))

### Changed
- Docs: expanded maintainer/release guidance and issue/label conventions. ([72870c2](https://github.com/nogcio/wrkr/commit/72870c2bd3d2ca1f9dfad5a1c76512af3b2d0d31), [b9613dc](https://github.com/nogcio/wrkr/commit/b9613dc48633b23ce7680f890e2a92f4ae14e7d3))

### Fixed
- CI: publish JUnit test reports via `mikepenz/action-junit-report`. ([51d6d5f](https://github.com/nogcio/wrkr/commit/51d6d5f19ab69a52d5e7bb97b837f39d6ac6c315))
- CI: allow reusable checks workflow to request `checks: write` and `pull-requests: read`. ([6a3798b](https://github.com/nogcio/wrkr/commit/6a3798bee00cf197b98391aeaf7e3af62dce40b9))
- CI: use system LuaJIT and `protoc` where appropriate. ([1c1d9a8](https://github.com/nogcio/wrkr/commit/1c1d9a80b95ccb5204d9fe1d95c653b3a2609521))
- CI: fix Windows `protoc` include discovery for bundling. ([d763754](https://github.com/nogcio/wrkr/commit/d763754e8764909b0fcf8b92e9e36ae12d211755))
- CI: fix Windows LuaJIT build by initializing MSVC environment. ([7575a72](https://github.com/nogcio/wrkr/commit/7575a7297cdf0b075f333f1cb38c5b5e0c399e5e))
- CI: fix Windows LuaJIT linking env export and verify `lua51.lib` is present. ([3b2e4db](https://github.com/nogcio/wrkr/commit/3b2e4dbfc061de9be11e338b6572a18b220d3c63))
- CI: fix Windows LuaJIT build step to `call msvcbuild.bat` so copy steps run. ([ce4dff5](https://github.com/nogcio/wrkr/commit/ce4dff5dcb71a1852f96a44b174254bbc0e5fc2a))
- CI: fix Windows linking by selecting MSVC `link.exe` (avoid Git `link.exe`). ([a014435](https://github.com/nogcio/wrkr/commit/a01443553e7c872c03a9c6c90669a66a5073c734))
- CI: fix Homebrew formula update script quoting. ([e9c53cb](https://github.com/nogcio/wrkr/commit/e9c53cbff92b51ef4944fd490a63f1a3f8dccd46))
- CI: fix macOS x86_64 build by using an Intel runner (avoid cross `pkg-config`/LuaJIT discovery issues). ([9589e38](https://github.com/nogcio/wrkr/commit/9589e384e99ff6cdfa6036eabdc8d1bd4cc4590f))

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
