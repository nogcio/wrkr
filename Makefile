.DEFAULT_GOAL := help

CARGO ?= cargo
CARGO_DENY ?= cargo deny

# Runtime defaults for `make run`
BASE_URL ?= http://127.0.0.1:12345
SCRIPT ?= examples/plaintext.lua
# Extra args passed to `wrkr run ...` (e.g. WRKR_RUN_ARGS='--vus 50 --duration 10s')
WRKR_RUN_ARGS ?=

# Python tooling (uv) configuration
UV ?= uv
# Use the workspace-level pyproject.toml at the repository root.
PY_PROJECT ?= .
# Extra args passed to the Python profiling tool (e.g. PROFILE_ARGS='--vus 512 --sample-duration 15')
PROFILE_ARGS ?=


.PHONY: help \
	fmt fmt-check lint test check \
	fmt-rust fmt-check-rust lint-rust test-rust check-rust \
	fmt-py fmt-check-py lint-py check-py \
	clippy build build-release run run-release testserver clean install-tools advisories docs docs-serve deps \
	py-sync py-lock py-fmt py-fmt-check py-lint py-check \
	py-test \
	tools-compare-perf-run \
	tools-profile-grpc tools-profile-wfb-grpc \
	tools-profile-grpc-aggregate-samply tools-profile-json-aggregate-samply

help: ## Show available targets
	@awk 'BEGIN {FS = ":.*##"} /^[a-zA-Z0-9_\-]+:.*##/ {printf "\033[36m%-24s\033[0m %s\n", $1, $2}' $(MAKEFILE_LIST)


fmt: fmt-rust fmt-py ## Format all code (Rust + Python)

fmt-rust: ## Format Rust code (cargo fmt)
	$(CARGO) fmt --all

fmt-check: fmt-check-rust fmt-check-py ## Verify formatting (Rust + Python)

fmt-check-rust: ## Verify Rust formatting (cargo fmt -- --check)
	$(CARGO) fmt --all -- --check

fmt-py: py-fmt ## Format Python code

fmt-check-py: py-fmt-check ## Verify Python formatting

lint: lint-rust lint-py ## Lint all code (Rust + Python)

lint-rust: clippy ## Lint Rust code (deny warnings)

lint-py: py-lint ## Lint Python code

clippy: ## Lint Rust code (deny warnings)
	$(CARGO) clippy --all-targets -- --deny warnings

test: test-rust ## Run tests (Rust)

test-rust: ## Run Rust tests (workspace)
	$(CARGO) test --workspace

build: ## Build debug (workspace)
	$(CARGO) build --workspace

build-release: ## Build release (workspace)
	$(CARGO) build --workspace --release

run: ## Run wrkr via cargo (SCRIPT=..., BASE_URL=..., WRKR_RUN_ARGS=...)
	BASE_URL="$(BASE_URL)" $(CARGO) run --bin wrkr -- run $(SCRIPT) $(WRKR_RUN_ARGS)

run-release: ## Run built release binary (requires build-release)
	BASE_URL="$(BASE_URL)" ./target/release/wrkr run $(SCRIPT) $(WRKR_RUN_ARGS)

testserver: ## Run local test server (prints BASE_URL)
	$(CARGO) run --bin wrkr-testserver

clean: ## Remove build artifacts
	$(CARGO) clean

install-tools: ## Install local CLI tools (cargo-deny, uv)
	@command -v cargo-deny >/dev/null 2>&1 || (echo "Installing cargo-deny..." && $(CARGO) install cargo-deny --locked)
	@command -v uv >/dev/null 2>&1 || (echo "uv not found. Install from https://docs.astral.sh/uv/getting-started/" && exit 1)

advisories: ## Check RustSec advisories (cargo-deny)
	@command -v cargo-deny >/dev/null 2>&1 || (echo "cargo-deny not found. Run 'make install-tools' first." && exit 1)
	$(CARGO_DENY) check advisories


check-rust: fmt-check-rust clippy test-rust advisories ## Run Rust checks (fmt + clippy + tests + advisories)

check-py: py-check ## Run Python checks (fmt + lint)

check: check-rust check-py ## Run checks (Rust + Python)

docs: ## Build documentation (mdBook)
	@command -v mdbook >/dev/null 2>&1 || (echo "mdbook not found. Install with: cargo install mdbook --locked" && exit 1)
	mdbook build docs

docs-serve: ## Serve documentation locally (mdBook)
	@command -v mdbook >/dev/null 2>&1 || (echo "mdbook not found. Install with: cargo install mdbook --locked" && exit 1)
	mdbook serve docs --open

deps: ## Install external deps (LuaJIT + protoc + uv) for local development
	@set -euo pipefail; \
	os="$$(uname -s 2>/dev/null || echo unknown)"; \
	case "$$os" in \
		Darwin) \
			command -v brew >/dev/null 2>&1 || (echo "Homebrew not found. Install from https://brew.sh/" && exit 1); \
			echo "Installing deps via Homebrew (luajit, pkg-config, protobuf, uv)..."; \
			brew install luajit pkg-config protobuf uv; \
			;; \
		Linux) \
			if command -v apt-get >/dev/null 2>&1; then \
				echo "Installing deps via apt-get (libluajit-5.1-dev, pkg-config, protobuf-compiler, python3-pip)..."; \
				sudo apt-get update; \
				sudo apt-get install -y libluajit-5.1-dev pkg-config protobuf-compiler python3-pip; \
				echo "Installing uv via pip..."; \
				pip3 install uv; \
			else \
				echo "Unsupported Linux distro (no apt-get found)."; \
				echo "Install manually:"; \
				echo "  - LuaJIT dev headers + pkg-config"; \
				echo "  - protoc (protobuf compiler)"; \
				echo "  - uv (Python package manager) from https://docs.astral.sh/uv/getting-started/"; \
				exit 1; \
			fi; \
		;; \
		*) \
			echo "Unsupported OS: $$os"; \
			echo "Install manually: LuaJIT (+ headers) + pkg-config + protoc + uv."; \
			exit 1; \
			;; \
	esac

# -------------------------
# Python tools (uv / ruff)
# -------------------------

py-sync: ## Sync Python environment for repo tools (uv)
	$(UV) sync --project $(PY_PROJECT) --all-extras

py-lock: ## Update uv.lock for repo tools (uv)
	$(UV) lock --project $(PY_PROJECT)

py-fmt: ## Format Python code (ruff format)
	$(UV) run --project $(PY_PROJECT) ruff format .

py-fmt-check: ## Check Python formatting (ruff format --check)
	$(UV) run --project $(PY_PROJECT) ruff format --check .

py-lint: ## Lint Python code (ruff check)
	$(UV) run --project $(PY_PROJECT) ruff check .

py-check: py-fmt-check py-lint py-test ## Run Python format check + lint + tests

py-test: ## Run Python unit tests (pytest)
	$(UV) run --project $(PY_PROJECT) pytest

# -------------------------
# Tools: compare-perf
# -------------------------

# Defaults for compare-perf (override on the make command line).
VUS ?= 64
DURATION ?= 5s

tools-compare-perf-run: ## Run compare-perf run
	# Defaults are intentionally conservative to avoid connection failures on laptops.
	# Override: `make tools-compare-perf-run VUS=256 DURATION=10s`
	$(UV) run --project $(PY_PROJECT) wrkr-tools-compare-perf run --build \
		--duration $(DURATION) \
		--wrkr-vus $(VUS) \
		--wrk-connections $(VUS) \
		--k6-vus $(VUS)

# -------------------------
# Tools: wrkr-tools-profile (CPU profiling)
# -------------------------
tools-profile-grpc-aggregate-samply: ## Profile grpc_aggregate via samply (PROFILE_ARGS=...)
	$(UV) run --project $(PY_PROJECT) wrkr-tools-profile grpc-aggregate-samply $(PROFILE_ARGS)

tools-profile-json-aggregate-samply: ## Profile json_aggregate via samply (PROFILE_ARGS=...)
	$(UV) run --project $(PY_PROJECT) wrkr-tools-profile json-aggregate-samply $(PROFILE_ARGS)