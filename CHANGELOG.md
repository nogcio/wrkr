# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/)
(and this file is parsed by the release workflow).

## [Unreleased]

### Added

### Changed

### Fixed

## [0.0.1] - 2026-01-20

### Added
- Lua scripting engine (`wrkr-lua`) with a small, module-based API (`require("wrkr/... ")`).
- Script contract: `options`, entrypoint `Default()`, and lifecycle hooks `Setup()`, `Teardown()`, `HandleSummary(summary)`.
- Executors: `constant-vus`, `ramping-vus`, `ramping-arrival-rate`.
- CLI overrides for runs: `--vus`, `--duration`, `--iterations`, `--env KEY=VALUE`, `--output`.
- Output formats: human summary and JSON progress lines (NDJSON).
- Built-in Lua modules: `wrkr/http`, `wrkr/check`, `wrkr/env`, `wrkr/json`, `wrkr/vu`, `wrkr/shared`, `wrkr/fs` (+ aggregate `wrkr`).
- `wrkr init` to scaffold a script workspace with `.luarc.json` and LuaLS stubs.
- Example scripts under `examples/` (HTTP, JSON aggregation, gRPC aggregation, lifecycle hooks).
- Distribution: GitHub Release binaries, Docker image on release tags, and Homebrew formula.
