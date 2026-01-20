# Contributing

Thank you for wanting to contribute! Please follow these guidelines to help us review and merge changes quickly.

## 1. Adding a New Benchmark

For a detailed step-by-step guide on adding new languages or frameworks, please read **[How to Add a New Benchmark](docs/GUIDE_ADDING_BENCHMARKS.md)**.

**Quick Summary:**
1.  **Create Directory**: `benchmarks/<language>/<framework>/` with a `Dockerfile` (port 8080).
2.  **Implement**: Follow specs in `docs/specs/*.md`.
3.  **Config**: Register in `config/benchmarks/<language>.yaml`, `config/frameworks.yaml`, and `config/languages.yaml`.
4.  **Verify**: Run locally with `cargo run --release --bin wfb-runner ...`.


## 2. General Workflow

1.  **Fork & Branch**:
    - Fork the repository.
    - Create a branch: `feat/add-framework-x`, `fix/issue-y`.

2.  **Code Style & Checks**:
    - **Rust Runner**: `cargo fmt --all` and `cargo clippy --all-targets -- -D warnings`.
    - **Testing**: Run `cargo test` to verify changes to the runner/server.

3.  **Commits & Pull Requests**:
    - Use clear, imperative commit messages (e.g., "Add Axum benchmark", "Fix CLI argument parsing").
    - Open a PR against `main` describing your changes.

## 3. PR Checklist

- [ ] Benchmark builds and runs locally (`cargo run --release --bin wfb-runner -- run <id> --env local`).
- [ ] Code is formatted and linters pass.
- [ ] `config/benchmarks/<language>.yaml` is updated correctly.

## 4. Communication & Security

- Do not commit secrets/tokens.
- Report security issues privately via `SECURITY.md`.
