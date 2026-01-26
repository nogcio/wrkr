#!/bin/sh

set -eu

WRKR_REPO_DEFAULT="nogcio/wrkr"

usage() {
  cat <<'EOF'
wrkr install script

Downloads a prebuilt wrkr binary from GitHub Releases and installs it into a directory on PATH.

USAGE:
  curl -fsSL https://raw.githubusercontent.com/nogcio/wrkr/main/install.sh | sh

OPTIONS:
  --version, -v   Version to install (tag). Examples: v0.2.3 or 0.2.3
                  Default: latest GitHub Release.
  --dir, -d       Install directory. Default:
                    - /usr/local/bin when run as root
                    - $HOME/.local/bin otherwise
  --repo          GitHub repo in owner/name form. Default: nogcio/wrkr
  --no-verify     Skip SHA256 verification.
  --help, -h      Show this help.

ENVIRONMENT:
  WRKR_VERSION      Same as --version
  WRKR_INSTALL_DIR  Same as --dir
  WRKR_INSTALL_REPO Same as --repo

NOTES:
  - wrkr links against system LuaJIT.
  - wrkr gRPC support may require protoc at runtime when loading .proto files.
EOF
}

log() {
  printf '%s\n' "$*" 1>&2
}

die() {
  log "error: $*"
  exit 1
}

need_cmd() {
  command -v "$1" >/dev/null 2>&1 || die "missing required command: $1"
}

get_os() {
  uname -s 2>/dev/null || die "uname -s failed"
}

get_arch() {
  uname -m 2>/dev/null || die "uname -m failed"
}

normalize_version_tag() {
  # Accept vX.Y.Z or X.Y.Z
  v="$1"
  case "$v" in
    v*) printf '%s' "$v" ;;
    '') printf '%s' "" ;;
    *) printf 'v%s' "$v" ;;
  esac
}

github_latest_tag() {
  repo="$1"
  need_cmd curl

  # Minimal JSON parsing without jq.
  tag=$(
    curl -fsSL "https://api.github.com/repos/${repo}/releases/latest" \
      | sed -n 's/.*"tag_name"[[:space:]]*:[[:space:]]*"\([^"]*\)".*/\1/p' \
      | head -n 1
  )

  [ -n "$tag" ] || die "failed to determine latest release tag from GitHub API (repo: $repo)"
  printf '%s' "$tag"
}

platform_target() {
  os="$1"
  arch="$2"

  case "$os" in
    Darwin)
      case "$arch" in
        arm64|aarch64) printf '%s' "aarch64-apple-darwin" ;;
        x86_64) printf '%s' "x86_64-apple-darwin" ;;
        *) die "unsupported macOS architecture: $arch" ;;
      esac
      ;;
    Linux)
      case "$arch" in
        x86_64) printf '%s' "x86_64-unknown-linux-gnu" ;;
        *) die "unsupported Linux architecture (no release artifact): $arch" ;;
      esac
      ;;
    *)
      die "unsupported OS: $os"
      ;;
  esac
}

mktemp_dir() {
  # macOS mktemp requires -d.
  d=$(mktemp -d 2>/dev/null || true)
  [ -n "$d" ] || die "failed to create temp directory"
  printf '%s' "$d"
}

sha256_of() {
  file="$1"

  if command -v sha256sum >/dev/null 2>&1; then
    sha256sum "$file" | awk '{print $1}'
    return 0
  fi

  if command -v shasum >/dev/null 2>&1; then
    shasum -a 256 "$file" | awk '{print $1}'
    return 0
  fi

  if command -v openssl >/dev/null 2>&1; then
    # Output: SHA2-256(file)= <hex>
    openssl dgst -sha256 "$file" | awk '{print $NF}'
    return 0
  fi

  die "no SHA256 tool found (need sha256sum, shasum, or openssl)"
}

extract_expected_sha256() {
  sumfile="$1"
  # Accept formats:
  #   <hex>  <filename>
  #   SHA2-256(<filename>)= <hex>
  expected=$(grep -Eo '[A-Fa-f0-9]{64}' "$sumfile" | head -n 1 || true)
  [ -n "$expected" ] || die "failed to parse SHA256 from $sumfile"
  printf '%s' "$expected"
}

main() {
  version="${WRKR_VERSION:-}"
  install_dir="${WRKR_INSTALL_DIR:-}"
  repo="${WRKR_INSTALL_REPO:-$WRKR_REPO_DEFAULT}"
  verify=1

  while [ "$#" -gt 0 ]; do
    case "$1" in
      -h|--help)
        usage
        exit 0
        ;;
      -v|--version)
        [ "$#" -ge 2 ] || die "--version requires an argument"
        version="$2"
        shift 2
        ;;
      -d|--dir)
        [ "$#" -ge 2 ] || die "--dir requires an argument"
        install_dir="$2"
        shift 2
        ;;
      --repo)
        [ "$#" -ge 2 ] || die "--repo requires an argument"
        repo="$2"
        shift 2
        ;;
      --no-verify)
        verify=0
        shift
        ;;
      *)
        die "unknown argument: $1 (try --help)"
        ;;
    esac
  done

  need_cmd uname
  need_cmd tar
  need_cmd awk
  need_cmd sed
  need_cmd head
  need_cmd grep
  need_cmd curl

  if [ -z "$install_dir" ]; then
    if [ "$(id -u 2>/dev/null || echo 1)" -eq 0 ]; then
      install_dir="/usr/local/bin"
    else
      install_dir="$HOME/.local/bin"
    fi
  fi

  os=$(get_os)
  arch=$(get_arch)
  target=$(platform_target "$os" "$arch")

  if [ -z "$version" ]; then
    version=$(github_latest_tag "$repo")
  else
    version=$(normalize_version_tag "$version")
  fi

  bin="wrkr"
  archive_base="${bin}-${version}-${target}"
  archive_file="${archive_base}.tar.gz"
  checksum_file="${archive_file}.sha256"
  base_url="https://github.com/${repo}/releases/download/${version}"

  tmpdir=$(mktemp_dir)
  cleanup() {
    rm -rf "$tmpdir" 2>/dev/null || true
  }
  trap cleanup EXIT INT TERM

  log "wrkr install"
  log "  repo:    $repo"
  log "  version: $version"
  log "  target:  $target"
  log "  dir:     $install_dir"

  log "Downloading ${archive_file}..."
  curl -fL --retry 3 --retry-delay 1 -o "$tmpdir/$archive_file" "${base_url}/${archive_file}"

  if [ "$verify" -eq 1 ]; then
    log "Downloading ${checksum_file}..."
    if curl -fsSL -o "$tmpdir/$checksum_file" "${base_url}/${checksum_file}"; then
      expected=$(extract_expected_sha256 "$tmpdir/$checksum_file")
      actual=$(sha256_of "$tmpdir/$archive_file")
      if [ "${expected}" != "${actual}" ]; then
        die "SHA256 mismatch for ${archive_file} (expected ${expected}, got ${actual})"
      fi
      log "SHA256 verified."
    else
      log "warning: checksum file not found; skipping verification"
    fi
  else
    log "warning: SHA256 verification disabled"
  fi

  log "Extracting..."
  tar -xzf "$tmpdir/$archive_file" -C "$tmpdir"

  [ -f "$tmpdir/$bin" ] || die "expected binary not found after extraction: $bin"

  mkdir -p "$install_dir"

  if command -v install >/dev/null 2>&1; then
    install -m 0755 "$tmpdir/$bin" "$install_dir/$bin"
  else
    cp "$tmpdir/$bin" "$install_dir/$bin"
    chmod 0755 "$install_dir/$bin"
  fi

  log "Installed: $install_dir/$bin"

  if ! command -v "$bin" >/dev/null 2>&1; then
    log "Note: '$install_dir' is not on your PATH."
    log "      You may want to add this to your shell profile:"
    log "      export PATH=\"$install_dir:\$PATH\""
  fi

  log ""
  log "Dependencies:"
  log "  - wrkr links against system LuaJIT"
  log "  - gRPC support may require protoc at runtime when loading .proto files"
  case "$os" in
    Darwin)
      if command -v brew >/dev/null 2>&1; then
        log "  macOS: brew install luajit"
        log "  macOS (gRPC protos): brew install protobuf"
      else
        log "  macOS: install LuaJIT (e.g. via Homebrew: brew install luajit)"
        log "  macOS (gRPC protos): install protoc (e.g. brew install protobuf)"
      fi
      ;;
    Linux)
      log "  Linux (Debian/Ubuntu): sudo apt-get install -y libluajit-5.1-2"
      log "  Linux (gRPC protos):   sudo apt-get install -y protobuf-compiler"
      ;;
  esac

  log "Done."
}

main "$@"
