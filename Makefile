.DEFAULT_GOAL := help

CARGO ?= cargo
CARGO_DENY ?= cargo deny

# Runtime defaults for `make run`
BASE_URL ?= http://127.0.0.1:12345
SCRIPT ?= examples/plaintext.lua
# Extra args passed to `wrkr run ...` (e.g. WRKR_RUN_ARGS='--vus 50 --duration 10s')
WRKR_RUN_ARGS ?=

.PHONY: help fmt fmt-check clippy test build build-release run run-release testserver clean check install-tools advisories docs docs-serve

help: ## Show available targets
	@awk 'BEGIN {FS = ":.*##"} /^[a-zA-Z0-9_\-]+:.*##/ {printf "\033[36m%-16s\033[0m %s\n", $$1, $$2}' $(MAKEFILE_LIST)

fmt: ## Format Rust code (cargo fmt)
	$(CARGO) fmt --all

fmt-check: ## Verify formatting (cargo fmt -- --check)
	$(CARGO) fmt --all -- --check

clippy: ## Lint Rust code (deny warnings)
	$(CARGO) clippy --all-targets -- --deny warnings

test: ## Run tests (workspace)
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

install-tools: ## Install local CLI tools (cargo-deny)
	@command -v cargo-deny >/dev/null 2>&1 || (echo "Installing cargo-deny..." && $(CARGO) install cargo-deny --locked)

advisories: ## Check RustSec advisories (cargo-deny)
	@command -v cargo-deny >/dev/null 2>&1 || (echo "cargo-deny not found. Run 'make install-tools' first." && exit 1)
	$(CARGO_DENY) check advisories

check: fmt-check clippy test advisories ## Run format check + clippy + tests + advisories

docs: ## Build documentation (mdBook)
	@command -v mdbook >/dev/null 2>&1 || (echo "mdbook not found. Install with: cargo install mdbook --locked" && exit 1)
	mdbook build docs

docs-serve: ## Serve documentation locally (mdBook)
	@command -v mdbook >/dev/null 2>&1 || (echo "mdbook not found. Install with: cargo install mdbook --locked" && exit 1)
	mdbook serve docs --open
