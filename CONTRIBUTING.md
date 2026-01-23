# Contributing

Thanks for contributing to `wrkr`!

`wrkr` is a Rust workspace with a small core runner (`wrkr-core`) and a Lua scripting engine (`wrkr-lua`). The best contributions are focused, well-tested, and keep performance in mind.

## Quick links

- Project overview + usage: `README.md`
- Lua scripting contract + built-in modules: `wrkr-lua/README.md`
- Security reporting: `.github/SECURITY.md`
- Code of conduct: `CODE_OF_CONDUCT.md`

## Development setup

Prerequisites:

- Rust toolchain (see `rust-toolchain.toml`)
- LuaJIT (system):
  - Linux (Debian/Ubuntu): `libluajit-5.1-dev` (and `pkg-config`)
  - macOS (Homebrew): `brew install luajit pkg-config`
- Protobuf compiler (`protoc`):
  - Linux (Debian/Ubuntu): `sudo apt-get install -y protobuf-compiler`
  - macOS (Homebrew): `brew install protobuf`

Common commands:

```bash
cargo fmt --all
cargo clippy --all-targets -- -D warnings
cargo test --workspace
```

Optional:

- Run the local test server (used by example scripts):

```bash
cargo run --bin wrkr-testserver
```

## Repo layout (high level)

- `wrkr/` — CLI binary (`wrkr`)
- `wrkr-core/` — core HTTP/gRPC runner and executors
- `wrkr-lua/` — Lua engine, built-in modules (`require("wrkr/...")`), and LuaLS stubs
- `wrkr-value/` — cross-language value contract
- `examples/` — ready-to-run scripts
- `docs/` — mdBook docs

## What to contribute

Good starter contributions:

- Fix bugs in CLI parsing / output formatting
- Improve docs (mdBook) and examples
- Add Lua module functionality (runtime + LuaLS stubs)
- Add tests for regressions

If you’re proposing a larger change (new executor, new scripting engine, protocol support), please open an issue first so we can align on API/architecture.

## Adding or changing Lua APIs

Rules (important):

- No global APIs in Lua: expose everything via `require("wrkr/... ")` modules.
- Keep runtime and editor stubs in sync:
  - Runtime implementation: `wrkr-lua/src/modules/*.rs` (registered via `package.preload`)
  - LuaLS stubs: `wrkr-lua/lua-stubs/wrkr/*.lua`

When you add a new module or function:

1) Implement it in Rust under `wrkr-lua/src/modules/`.
2) Add/adjust Lua stubs so LuaLS shows correct signatures.
3) Add tests in `wrkr-lua/tests/` (prefer unit/integration tests over manual scripts).
4) Update docs in `wrkr-lua/README.md` and/or mdBook docs if user-facing.

## Code style & quality bar

- Keep it simple (KISS) and avoid duplication (DRY).
- Prefer small, composable modules; make logic testable.
- Performance matters: avoid unnecessary allocations and cloning.
- Clippy warnings are treated as errors in CI.

## Pull request workflow

1) Fork + create a branch:

- `feat/<short-name>` for features
- `fix/<short-name>` for bugfixes

2) Make the change + add/adjust tests.

3) Run checks locally:

```bash
cargo fmt --all
cargo clippy --all-targets -- -D warnings
cargo test --workspace
```

4) Open a PR against `main`.

## Issues, triage, and milestones

- Please use the issue templates (Bug report / Feature request / Documentation request).
- For usage questions (“how do I…?”), prefer GitHub Discussions.
- Maintainers will apply priority/area labels and assign milestones.
- If you want to work on an issue, comment on it first so we can align on approach and scope.

Maintainer-facing workflow notes live in `.github/MAINTAINER_GUIDE.md`.

For the recommended label set and release steps:
- `.github/LABELS.md`
- `RELEASING.md`

## Release/versioning notes

Releases are cut from tags `vX.Y.Z`. CI builds binaries, publishes Docker images, and updates the Homebrew formula (see workflows under `.github/workflows/`).

## Security

Please do not open public issues for security vulnerabilities. Use the process in `.github/SECURITY.md`.
