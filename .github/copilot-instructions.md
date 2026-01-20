
# Project development principles

- **DRY**: avoid duplicating logic; extract shared code into reusable functions/modules.
- **KISS**: choose the simplest solution that correctly meets the requirements.
- **Maintainability and extensibility first**: changes should be clear, localized, and safe.
- **No backward compatibility by default**: when we refactor or redesign behavior, we do not preserve backwards compatibility unless explicitly requested; remove obsolete APIs, code paths, flags, and related tests/docs.
- **Testability first**: favor small, composable modules and dependency injection (or clean seams) so core logic can be unit-tested without heavy I/O or runtime coupling; when implementing behavior, add/adjust tests to cover it.
- **Tests: no inline scripts**: never embed scripts inline in tests; scripts must always live in dedicated files (checked into the repo) and tests should load them from disk.
- **Performance is critical**: prefer efficient algorithms/data structures and avoid unnecessary work.
- **No cutting corners**: if something is unclear or doesn’t work, read the docs/sources instead of adding brittle hacks.
- **Keep documentation up to date**: update relevant docs/README/architecture notes alongside code changes so documentation reflects the current behavior.
- **Modularity and small files**: don’t pile everything into one place; keep code split into focused modules with clear boundaries.
- **Modularity is non-negotiable**: different entities belong in different files/modules; when introducing a new “thing” (type/module/API), create a dedicated file/module for it rather than appending it to an unrelated one.
- **Idiomatic Rust & best practices**: keep the project idiomatic and follow Rust best practices (standard tooling like `cargo fmt`/`clippy`, clear ownership/lifetimes, avoid `unsafe` unless justified, and prefer conventional crate/module organization).

- **Rust module layout (single style)**: use `foo.rs` for modules. If a module has submodules, keep the parent as `foo.rs` and place children in `foo/*.rs`. Do not use `foo/mod.rs` anywhere in this repo.

- **Cross-language data contract**: if arbitrary values must cross the boundary between a scripting language (Lua/JS/etc) and `wrkr-core`, use `wrkr-value` (`wrkr_value::Value`/`MapKey`). Do not pass `prost_reflect` types, `serde_json::Value`, or ad-hoc strings as the interop format.

- **Use enums for fixed sets**: if a value is conceptually a fixed/closed enumeration (e.g. request kind, protocol mode, known status bucket), represent it as a Rust `enum` (with a stable string mapping when needed for tags/metrics) rather than passing ad-hoc strings.

- **Enum string conversions must be centralized**: if an enum needs to be parsed from a string and/or rendered as a stable string (CLI, config, tags/metrics), define that mapping on the enum itself using derive macros (prefer `strum` with `EnumString`/`Display` and explicit `#[strum(serialize = "...")]` aliases). Do not scatter `match`/`if` chains converting enums to/from strings across the project.

- **Lua runner API rules**:
	- **No globals**: do not expose `http`, `check`, `__ENV`, `open`, etc. as Lua globals.
	- **Modules only**: expose all Lua APIs via `require("wrkr/... ")` style modules (e.g. `wrkr/http`, `wrkr/check`, `wrkr/env`, `wrkr/fs`).

- **Lua + LuaLS stubs must be warning-free**:
	- When editing or adding files under `wrkr-lua/lua-stubs/`, ensure LuaLS (sumneko) produces no diagnostics.
	- Avoid conflicting declarations (e.g. don’t define `M.Client` as both a function and a table; model submodules as separate `*Module` tables when needed).
	- If a stub has `---@return`, provide a dummy return value of the correct type to avoid `missing-return` warnings.
	- Keep signatures aligned with runtime behavior (e.g. `wrkr/check` accepts any value, not only HTTP responses).

- **Lua integration (how it works in this repo)**:
	- **Where the Lua integration lives**:
		- Runtime implementation: `wrkr-lua/src/*` (entrypoints: `parse_script_options`, `run_vu`).
		- Built-in Lua modules are implemented in Rust under `wrkr-lua/src/modules/*.rs` and are registered via `package.preload`.
		- LuaLS/editor stubs for the built-in modules live under `wrkr-lua/lua-stubs/wrkr`.
		- User-facing docs for the script contract and modules are in `wrkr-lua/README.md`.
	- **Module model**:
		- Built-ins are only available via `require("wrkr/... ")` (plus the convenience aggregate `require("wrkr")`).
		- The runner prepends the script directory to `package.path` so scripts can `require()` local files next to the script (`?.lua` and `?/init.lua`).
		- Optional external module paths can be prepended via `WRKR_LUA_PATH`/`LUA_PATH` and `WRKR_LUA_CPATH`/`LUA_CPATH`.
	- **Execution flow (two-phase)**:
		- Phase 1: parse options.
			- The CLI reads the script and runs `wrkr_lua::parse_script_options(...)` in a dedicated Lua state.
			- The script is executed once to read global `options` and (optionally) `options.scenarios`.
		- Phase 2: run scenarios.
			- Scenarios are derived from options (`wrkr_core::runner::scenarios_from_options`) and then executed (`wrkr_core::runner::run_scenarios`).
			- Each VU runs `wrkr_lua::run_vu(...)`, which creates its own Lua state, registers the `wrkr/*` modules, executes the script, then repeatedly calls the selected entry function (`Default()` or `exec` per scenario).
	- **Sandboxing / limits**:
		- `wrkr` does not implement a CPU/time/memory sandbox for Lua scripts (a script can busy-loop).
		- By default we create Lua via `mlua::Lua::new()` (mlua “safe” mode; notably it does not load the `debug` standard library).
		- When debugging is enabled, we switch to `mlua::Lua::unsafe_new()` to make the `debug` stdlib available for `lldebugger`.

- **Validation workflow**: before handing off a change, run formatting, linting, and tests to ensure correctness: `cargo fmt --all`, `cargo clippy --all-targets -- -D warnings`, and `cargo test --workspace` (or the narrowest applicable subset).

- **Rust 2024 + clippy is the contract**: treat `cargo clippy --all-targets -- -D warnings` as a hard requirement. If clippy would warn, consider the change broken.

- **Clippy-clean by default (common pitfalls to avoid)**:
	- **No unused things**: avoid unused imports/variables/`mut`/dead code; prefer `_var` for intentionally-unused bindings.
	- **Prefer inlined format args**: write `format!("{value}")` / `println!("{value:?}")` instead of `format!("{}", value)` to avoid `clippy::uninlined_format_args`.
	- **Avoid needless conversions/borrows**: don’t add `.to_string()`/`.to_owned()`/`.clone()` unless necessary; don’t write `&x` or `*x` when the type already matches.
	- **Avoid needless control flow**: no `return` at end of blocks, no `else {}` after an early `return`, avoid `let x = expr; x` when `expr` can be returned directly.
	- **Idiomatic bools/options/results**: prefer `is_some_and`, `map_or`, `unwrap_or_else`, `ok_or_else`, and `matches!` where it’s clearer/shorter.
	- **Don’t ignore fallible results**: handle `Result` and `Option` explicitly; only use `unwrap/expect` in tests (and with clear messages when used).

- **Errors & Results**: in library crates (e.g. `wrkr-core`, `wrkr-lua`) prefer a crate-local typed `Error` and `Result<T, Error>` (with idiomatic conversions via `From`/`thiserror`-style patterns). In binaries (e.g. `wrkr`) it’s OK to use `anyhow::Result` / `anyhow::Error` for top-level application wiring and rich context.

