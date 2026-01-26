#!/usr/bin/env bash
set -euo pipefail

cd "${WORKSPACE_FOLDER:-$(pwd)}"

export PATH="$HOME/.cargo/bin:$HOME/.local/bin:$PATH"

echo "==> Rust toolchain"
rustup show

echo "==> Cargo deps (fetch)"
cargo fetch

echo "==> Coverage tooling (llvm-tools + cargo-llvm-cov)"
rustup component add llvm-tools-preview >/dev/null
if ! command -v cargo-llvm-cov >/dev/null 2>&1; then
  cargo install cargo-llvm-cov --locked
fi

echo "==> Profiling tooling (inferno for flamegraphs)"
if ! command -v inferno-flamegraph >/dev/null 2>&1; then
  cargo install inferno --locked
fi

echo "==> Profiling tooling (samply)"
if ! command -v samply >/dev/null 2>&1; then
  cargo install samply --locked
fi

if ! command -v cargo-deny >/dev/null 2>&1; then
  echo "==> Installing cargo-deny"
  cargo install cargo-deny --locked
fi

if ! command -v mdbook >/dev/null 2>&1; then
  echo "==> Installing mdbook"
  cargo install mdbook --locked
fi

if ! command -v k6 >/dev/null 2>&1; then
  echo "==> Installing k6 (for wrkr-tools-compare-perf)"
  arch="$(uname -m)"
  case "$arch" in
    x86_64) k6_arch="amd64" ;;
    aarch64|arm64) k6_arch="arm64" ;;
    *)
      echo "Unsupported arch for k6: $arch (skipping)" >&2
      k6_arch=""
      ;;
  esac

  if [ -n "$k6_arch" ]; then
    # Prefer latest release; fall back to a pinned version if GitHub API is unavailable.
    k6_ver="${K6_VERSION:-}"
    if [ -z "$k6_ver" ]; then
      k6_ver="$(curl -fsSL https://api.github.com/repos/grafana/k6/releases/latest \
        | python3 -c 'import json,sys; print(json.load(sys.stdin)["tag_name"].lstrip("v"))' \
        || true)"
    fi
    if [ -z "$k6_ver" ]; then
      k6_ver="1.5.0"
    fi

    tmp_dir="$(mktemp -d)"
    trap 'rm -rf "$tmp_dir"' EXIT
    url="https://github.com/grafana/k6/releases/download/v${k6_ver}/k6-v${k6_ver}-linux-${k6_arch}.tar.gz"
    curl -fsSL "$url" -o "$tmp_dir/k6.tgz"
    tar -xzf "$tmp_dir/k6.tgz" -C "$tmp_dir"
    install -m 0755 "$tmp_dir/k6-v${k6_ver}-linux-${k6_arch}/k6" "$HOME/.local/bin/k6"
    echo "Installed k6: $($HOME/.local/bin/k6 version | head -n 1)"
  fi
fi

if command -v uv >/dev/null 2>&1; then
  echo "==> Python tooling env (uv sync --all-extras)"
  uv sync --project . --all-extras
else
  echo "==> uv not found; skipping Python tooling setup"
fi
