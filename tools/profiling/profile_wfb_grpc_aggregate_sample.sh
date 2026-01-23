#!/usr/bin/env bash
set -euo pipefail

root_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
cd "$root_dir"

# Build optimized binaries with debuginfo (so samples have symbols).
cargo build --profile profiling >/dev/null

# Start testserver on ephemeral ports.
# It prints BASE_URL=... and GRPC_TARGET=... to stdout once ready.
server_out="$(mktemp -t wrkr-testserver.XXXXXX)"
"$root_dir/target/profiling/wrkr-testserver" >"$server_out" 2>&1 &
server_pid=$!

cleanup() {
  kill "$server_pid" >/dev/null 2>&1 || true
  wait "$server_pid" >/dev/null 2>&1 || true
  rm -f "$server_out" >/dev/null 2>&1 || true
}
trap cleanup EXIT

# Wait until we have GRPC_TARGET.
GRPC_TARGET=""
for _ in {1..200}; do
  if grep -q '^GRPC_TARGET=' "$server_out"; then
    GRPC_TARGET="$(grep '^GRPC_TARGET=' "$server_out" | tail -n1 | cut -d= -f2-)"
    break
  fi
  sleep 0.05
done

if [[ -z "$GRPC_TARGET" ]]; then
  echo "ERROR: failed to get GRPC_TARGET from wrkr-testserver" >&2
  echo "--- testserver output ---" >&2
  cat "$server_out" >&2
  exit 1
fi

echo "GRPC_TARGET=$GRPC_TARGET"

# Warmup (avoids one-time costs dominating the sample).
"$root_dir/target/profiling/wrkr" run tools/perf/wrkr_grpc_plaintext.lua --duration 1s --vus 64 --env "GRPC_TARGET=$GRPC_TARGET" >/dev/null

mkdir -p "$root_dir/tmp"

# How long to sample stack traces for.
sample_duration_seconds="${1:-10}"
# How long to keep the load test running.
load_duration="${2:-30s}"
# How many VUs to run.
vus="${3:-50}"
# Delay before starting sampling (helps avoid startup / proto compilation skew).
pre_sample_sleep_seconds="${4:-5}"

sample_out="$root_dir/tmp/wfb_grpc_aggregate_sample_${sample_duration_seconds}s.txt"
rm -f "$sample_out"

# Run wrkr in background so we can sample its stacks.
"$root_dir/target/profiling/wrkr" run tools/perf/wfb_grpc_aggregate.lua \
  --duration "$load_duration" \
  --vus "$vus" \
  --env "GRPC_TARGET=$GRPC_TARGET" \
  >/dev/null &
wrkr_pid=$!

sleep "$pre_sample_sleep_seconds"

# If wrkr exits early, sampling will fail; that's fine (we'll surface the error).
# macOS 'sample' captures stack traces periodically for the given duration.
set +e
sample "$wrkr_pid" "$sample_duration_seconds" -file "$sample_out" >/dev/null 2>&1
sample_status=$?
set -e

wait "$wrkr_pid" || true

if [[ $sample_status -ne 0 ]]; then
  echo "ERROR: 'sample' failed (exit $sample_status)." >&2
  echo "Try running: sample $wrkr_pid $sample_duration_seconds -file $sample_out" >&2
  exit 2
fi

echo "Sample written to: $sample_out"
echo "Top hint: search for 'Call graph:' and 'Heaviest stack' inside that file."
