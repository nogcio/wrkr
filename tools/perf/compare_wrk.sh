#!/usr/bin/env bash
set -euo pipefail

DURATION="${DURATION:-5s}"
WRK_CONNECTIONS="${WRK_CONNECTIONS:-256}"
WRK_THREADS="${WRK_THREADS:-8}"
WRKR_VUS="${WRKR_VUS:-256}"
K6_VUS="${K6_VUS:-$WRKR_VUS}"
RATIO_OK="${RATIO_OK:-0.95}" # GET /hello: wrkr_rps must be >= wrk_rps * RATIO_OK
RATIO_OK_POST_JSON="${RATIO_OK_POST_JSON:-0.90}" # POST /echo: wrkr_rps must be >= wrk_rps * RATIO_OK_POST_JSON
RATIO_OK_WRKR_OVER_K6="${RATIO_OK_WRKR_OVER_K6:-1.5}" # wrkr_rps must be > k6_rps * RATIO_OK_WRKR_OVER_K6 (default: >1.0)
NATIVE="${NATIVE:-1}"       # if 1, build with -C target-cpu=native (best perf, machine-specific)

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"

need_cmd() {
  if ! command -v "$1" >/dev/null 2>&1; then
    echo "missing required command: $1" >&2
    return 1
  fi
}

need_cmd cargo
need_cmd awk
need_cmd sed
need_cmd grep
need_cmd mktemp
need_cmd uname

# Bash 3.2 compatibility (macOS default): no associative arrays.
set_var() {
  local name="$1"
  local value="$2"
  printf -v "$name" '%s' "$value"
}

TIME_BIN="/usr/bin/time"
if [[ ! -x "$TIME_BIN" ]]; then
  echo "missing required binary: $TIME_BIN" >&2
  exit 2
fi

OS_NAME="$(uname -s)"
TIME_ARGS=()
case "$OS_NAME" in
  Darwin)
    TIME_ARGS=(-l)
    ;;
  Linux)
    TIME_ARGS=(-v)
    ;;
  *)
    echo "unsupported OS for RSS measurement: $OS_NAME" >&2
    echo "supported: Darwin, Linux" >&2
    exit 2
    ;;
esac

parse_time_rss_bytes() {
  local time_file="$1"

  # macOS: "maximum resident set size  12345678" (bytes)
  if grep -q "maximum resident set size" "$time_file"; then
    # /usr/bin/time -l format:
    #   <bytes>  maximum resident set size
    awk '/maximum resident set size/ {print $1; exit}' "$time_file"
    return 0
  fi

  # Linux: "Maximum resident set size (kbytes): 12345" (kbytes)
  if grep -q "Maximum resident set size" "$time_file"; then
    awk -F':' '/Maximum resident set size/ {
      gsub(/^[[:space:]]+|[[:space:]]+$/, "", $2);
      printf("%d\n", ($2 + 0) * 1024);
      exit
    }' "$time_file"
    return 0
  fi

  echo "0"
}

bytes_to_mb() {
  local bytes="$1"
  awk -v b="$bytes" 'BEGIN { printf("%.2f", (b + 0.0) / 1024 / 1024) }'
}

parse_wrkr_rps_from_output() {
  local out="$1"

  # Legacy format: "rps: 1234"
  local rps
  rps="$(echo "$out" | awk '/^rps:/ {print $2}' | tail -n1)"
  if [[ -n "$rps" ]]; then
    echo "$rps"
    return 0
  fi

  # Current format (k6-like summary):
  #   http_reqs.......................: 1085653 (217130.60000/s)
  # Prefer http_reqs, fallback to iterations.
  rps="$(
    echo "$out" | awk '
      /http_reqs|iterations/ {
        for (i=1;i<=NF;i++) {
          if ($i ~ /\/s\)$/) {
            gsub(/[()]/, "", $i);
            gsub(/\/s$/, "", $i);
            print $i;
            exit
          }
        }
      }
    ' | head -n1
  )"
  if [[ -n "$rps" ]]; then
    echo "$rps"
    return 0
  fi

  return 1
}

measure_wrk() {
  local script_path="$1"
  local base_url="$2"

  WRK_TIME_FILE="$(mktemp -t wrk.time.XXXXXX)"
  WRK_OUT_FILE="$(mktemp -t wrk.out.XXXXXX)"
  "$TIME_BIN" "${TIME_ARGS[@]}" wrk -t"$WRK_THREADS" -c"$WRK_CONNECTIONS" -d"$DURATION" -s "$script_path" "$base_url" >"$WRK_OUT_FILE" 2>"$WRK_TIME_FILE"
  WRK_OUT="$(cat "$WRK_OUT_FILE")"
  WRK_RSS_BYTES="$(parse_time_rss_bytes "$WRK_TIME_FILE")"
  rm -f "$WRK_TIME_FILE" "$WRK_OUT_FILE"

  WRK_RPS="$(echo "$WRK_OUT" | awk '/Requests\/sec:/ {print $2}' | head -n1)"
  if [[ -z "$WRK_RPS" ]]; then
    echo "failed to parse wrk RPS" >&2
    exit 1
  fi
}

cache_has() {
  local name="$1"
  [[ -n "${!name-}" ]]
}

cache_set() {
  local name="$1"
  local value="$2"
  set_var "$name" "$value"
}

cache_get() {
  local name="$1"
  printf '%s' "${!name-}"
}

ensure_wrk_cached() {
  local cache_key="$1"   # e.g. HELLO_BENCH
  local script_path="$2"
  local base_url="$3"

  if cache_has "${cache_key}_WRK_RPS"; then
    WRK_RPS="$(cache_get "${cache_key}_WRK_RPS")"
    WRK_RSS_BYTES="$(cache_get "${cache_key}_WRK_RSS_BYTES")"
    WRK_OUT="$(cache_get "${cache_key}_WRK_OUT")"
    return 0
  fi

  measure_wrk "$script_path" "$base_url"
  cache_set "${cache_key}_WRK_RPS" "$WRK_RPS"
  cache_set "${cache_key}_WRK_RSS_BYTES" "$WRK_RSS_BYTES"
  cache_set "${cache_key}_WRK_OUT" "$WRK_OUT"
}

measure_wrkr() {
  local script_rel="$1"
  local base_url="$2"

  WRKR_TIME_FILE="$(mktemp -t wrkr.time.XXXXXX)"
  WRKR_OUT_FILE="$(mktemp -t wrkr.out.XXXXXX)"
  (cd "$ROOT_DIR" && "$TIME_BIN" "${TIME_ARGS[@]}" "$ROOT_DIR/target/release/wrkr" run "$script_rel" --duration "$DURATION" --vus "$WRKR_VUS" --env "BASE_URL=$base_url") >"$WRKR_OUT_FILE" 2>"$WRKR_TIME_FILE"
  WRKR_OUT="$(cat "$WRKR_OUT_FILE")"
  WRKR_RSS_BYTES="$(parse_time_rss_bytes "$WRKR_TIME_FILE")"
  rm -f "$WRKR_TIME_FILE" "$WRKR_OUT_FILE"

  if ! WRKR_RPS="$(parse_wrkr_rps_from_output "$WRKR_OUT")"; then
    echo "failed to parse wrkr RPS" >&2
    echo "--- wrkr output (for debugging) ---" >&2
    echo "$WRKR_OUT" >&2
    exit 1
  fi
}

measure_k6() {
  local script_path="$1"
  local base_url="$2"

  K6_TIME_FILE="$(mktemp -t k6.time.XXXXXX)"
  K6_OUT_FILE="$(mktemp -t k6.out.XXXXXX)"

  # k6 writes progress to stderr; we capture stderr into the time file (it will also include /usr/bin/time output).
  BASE_URL="$base_url" "$TIME_BIN" "${TIME_ARGS[@]}" k6 run --vus "$K6_VUS" --duration "$DURATION" "$script_path" >"$K6_OUT_FILE" 2>"$K6_TIME_FILE"
  K6_OUT="$(cat "$K6_OUT_FILE")"
  K6_RSS_BYTES="$(parse_time_rss_bytes "$K6_TIME_FILE")"
  rm -f "$K6_TIME_FILE" "$K6_OUT_FILE"

  # Example line:
  #   http_reqs......................: 12345  1234.5/s
  K6_RPS="$(echo "$K6_OUT" | awk '/http_reqs/ {
    for (i=1;i<=NF;i++) {
      if (index($i, "/s") > 0) {
        gsub(/\/s/, "", $i);
        print $i;
        exit
      }
    }
  }')"
  if [[ -z "$K6_RPS" ]]; then
    echo "failed to parse k6 RPS" >&2
    exit 1
  fi
}

ensure_k6_cached() {
  local cache_key="$1"   # e.g. HELLO_BENCH
  local script_path="$2"
  local base_url="$3"

  if [[ "${HAS_K6:-0}" != "1" ]]; then
    return 0
  fi

  if cache_has "${cache_key}_K6_RPS"; then
    K6_RPS="$(cache_get "${cache_key}_K6_RPS")"
    K6_RSS_BYTES="$(cache_get "${cache_key}_K6_RSS_BYTES")"
    K6_OUT="$(cache_get "${cache_key}_K6_OUT")"
    return 0
  fi

  measure_k6 "$script_path" "$base_url"
  cache_set "${cache_key}_K6_RPS" "$K6_RPS"
  cache_set "${cache_key}_K6_RSS_BYTES" "$K6_RSS_BYTES"
  cache_set "${cache_key}_K6_OUT" "$K6_OUT"
}

FAILURES=0

capture_case() {
  local case_key="$1"   # e.g. HELLO, POST_JSON
  local ratio_ok="$2"

  local wrk_rss_mb
  local wrkr_rss_mb
  wrk_rss_mb="$(bytes_to_mb "$WRK_RSS_BYTES")"
  wrkr_rss_mb="$(bytes_to_mb "$WRKR_RSS_BYTES")"

  local rps_ratio_wrkr_over_wrk
  local mem_ratio_wrkr_over_wrk
  rps_ratio_wrkr_over_wrk="$(awk -v a="$WRKR_RPS" -v b="$WRK_RPS" 'BEGIN { if ((b + 0.0) <= 0.0) { print "-" } else { printf("%.3f", (a + 0.0) / (b + 0.0)) } }')"
  mem_ratio_wrkr_over_wrk="$(awk -v a="$WRKR_RSS_BYTES" -v b="$WRK_RSS_BYTES" 'BEGIN { if ((b + 0.0) <= 0.0) { print "-" } else { printf("%.3f", (a + 0.0) / (b + 0.0)) } }')"

  local status
  local ok_wrkr_over_wrk
  local ok_wrkr_over_k6
  local check_wrkr_over_wrk_status
  local check_wrkr_over_k6_status

  ok_wrkr_over_wrk="$(awk -v wrk="$WRK_RPS" -v wrkr="$WRKR_RPS" -v ratio="$ratio_ok" 'BEGIN { if (wrkr + 0.0 > (wrk + 0.0) * (ratio + 0.0)) print "yes"; else print "no" }')"

  if [[ "${HAS_K6:-0}" != "1" ]]; then
    ok_wrkr_over_k6="no"
  else
    ok_wrkr_over_k6="$(awk -v wrkr="$WRKR_RPS" -v k6="$K6_RPS" -v ratio="$RATIO_OK_WRKR_OVER_K6" 'BEGIN { if (wrkr + 0.0 > (k6 + 0.0) * (ratio + 0.0)) print "yes"; else print "no" }')"
  fi

  if [[ "$ok_wrkr_over_wrk" == "yes" ]]; then
    check_wrkr_over_wrk_status="PASS"
  else
    check_wrkr_over_wrk_status="FAIL"
  fi
  if [[ "$ok_wrkr_over_k6" == "yes" ]]; then
    check_wrkr_over_k6_status="PASS"
  else
    check_wrkr_over_k6_status="FAIL"
  fi

  if [[ "$ok_wrkr_over_wrk" == "yes" && "$ok_wrkr_over_k6" == "yes" ]]; then
    status="PASS"
  else
    status="FAIL"
    FAILURES=$((FAILURES + 1))
  fi

  set_var "${case_key}_WRK_RPS" "$WRK_RPS"
  set_var "${case_key}_WRKR_RPS" "$WRKR_RPS"
  set_var "${case_key}_WRK_RSS_MB" "$wrk_rss_mb"
  set_var "${case_key}_WRKR_RSS_MB" "$wrkr_rss_mb"
  set_var "${case_key}_RPS_RATIO_WRKR_OVER_WRK" "$rps_ratio_wrkr_over_wrk"
  set_var "${case_key}_MEM_RATIO_WRKR_OVER_WRK" "$mem_ratio_wrkr_over_wrk"
  set_var "${case_key}_GATE_RATIO_OK" "$ratio_ok"
  set_var "${case_key}_GATE_STATUS" "$status"
  set_var "${case_key}_CHECK_WRKR_OVER_WRK_STATUS" "$check_wrkr_over_wrk_status"
  set_var "${case_key}_CHECK_WRKR_OVER_K6_STATUS" "$check_wrkr_over_k6_status"

  if [[ "${HAS_K6:-0}" == "1" ]]; then
    local k6_rss_mb
    k6_rss_mb="$(bytes_to_mb "$K6_RSS_BYTES")"
    local rps_ratio_k6_over_wrk
    local rps_ratio_wrkr_over_k6
    local mem_ratio_k6_over_wrk
    local mem_ratio_wrkr_over_k6
    rps_ratio_k6_over_wrk="$(awk -v a="$K6_RPS" -v b="$WRK_RPS" 'BEGIN { if ((b + 0.0) <= 0.0) { print "-" } else { printf("%.3f", (a + 0.0) / (b + 0.0)) } }')"
    rps_ratio_wrkr_over_k6="$(awk -v a="$WRKR_RPS" -v b="$K6_RPS" 'BEGIN { if ((b + 0.0) <= 0.0) { print "-" } else { printf("%.3f", (a + 0.0) / (b + 0.0)) } }')"
    mem_ratio_k6_over_wrk="$(awk -v a="$K6_RSS_BYTES" -v b="$WRK_RSS_BYTES" 'BEGIN { if ((b + 0.0) <= 0.0) { print "-" } else { printf("%.3f", (a + 0.0) / (b + 0.0)) } }')"
    mem_ratio_wrkr_over_k6="$(awk -v a="$WRKR_RSS_BYTES" -v b="$K6_RSS_BYTES" 'BEGIN { if ((b + 0.0) <= 0.0) { print "-" } else { printf("%.3f", (a + 0.0) / (b + 0.0)) } }')"

    set_var "${case_key}_K6_RPS" "$K6_RPS"
    set_var "${case_key}_K6_RSS_MB" "$k6_rss_mb"
    set_var "${case_key}_RPS_RATIO_K6_OVER_WRK" "$rps_ratio_k6_over_wrk"
    set_var "${case_key}_RPS_RATIO_WRKR_OVER_K6" "$rps_ratio_wrkr_over_k6"
    set_var "${case_key}_MEM_RATIO_K6_OVER_WRK" "$mem_ratio_k6_over_wrk"
    set_var "${case_key}_MEM_RATIO_WRKR_OVER_K6" "$mem_ratio_wrkr_over_k6"
  else
    set_var "${case_key}_K6_RPS" "-"
    set_var "${case_key}_K6_RSS_MB" "-"
    set_var "${case_key}_RPS_RATIO_K6_OVER_WRK" "-"
    set_var "${case_key}_RPS_RATIO_WRKR_OVER_K6" "-"
    set_var "${case_key}_MEM_RATIO_K6_OVER_WRK" "-"
    set_var "${case_key}_MEM_RATIO_WRKR_OVER_K6" "-"
  fi
}

print_summary() {
  get_var() {
    local name="$1"
    printf '%s' "${!name-}"
  }

  print_case_table() {
    local title="$1"
    local key="$2"

    local wrk_rps
    local wrkr_rps
    local k6_rps
    local wrk_mb
    local wrkr_mb
    local k6_mb
    local ratio_ok
    local gate
    local rps_wrkr_over_wrk
    local rps_k6_over_wrk
    local rps_wrkr_over_k6
    local mem_wrkr_over_wrk
    local mem_k6_over_wrk
    local mem_wrkr_over_k6

    wrk_rps="$(get_var "${key}_WRK_RPS")"
    wrkr_rps="$(get_var "${key}_WRKR_RPS")"
    k6_rps="$(get_var "${key}_K6_RPS")"
    wrk_mb="$(get_var "${key}_WRK_RSS_MB")"
    wrkr_mb="$(get_var "${key}_WRKR_RSS_MB")"
    k6_mb="$(get_var "${key}_K6_RSS_MB")"
    ratio_ok="$(get_var "${key}_GATE_RATIO_OK")"
    gate="$(get_var "${key}_GATE_STATUS")"
    rps_wrkr_over_wrk="$(get_var "${key}_RPS_RATIO_WRKR_OVER_WRK")"
    rps_k6_over_wrk="$(get_var "${key}_RPS_RATIO_K6_OVER_WRK")"
    rps_wrkr_over_k6="$(get_var "${key}_RPS_RATIO_WRKR_OVER_K6")"
    mem_wrkr_over_wrk="$(get_var "${key}_MEM_RATIO_WRKR_OVER_WRK")"
    mem_k6_over_wrk="$(get_var "${key}_MEM_RATIO_K6_OVER_WRK")"
    mem_wrkr_over_k6="$(get_var "${key}_MEM_RATIO_WRKR_OVER_K6")"

    if [[ "${HAS_K6:-0}" != "1" ]]; then
      k6_rps="-"
      k6_mb="-"
      rps_k6_over_wrk="-"
      rps_wrkr_over_k6="-"
      mem_k6_over_wrk="-"
      mem_wrkr_over_k6="-"
    fi

    echo
    echo "===================="
    echo "CASE SUMMARY: $title"
    echo "===================="
    printf '%-28s | %12s | %12s | %12s\n' "metric" "wrk" "wrkr" "k6"
    printf '%-28s-+-%12s-+-%12s-+-%12s\n' "----------------------------" "------------" "------------" "------------"

    printf '%-28s | %12s | %12s | %12s\n' "rps" "$wrk_rps" "$wrkr_rps" "$k6_rps"
    printf '%-28s | %12s | %12s | %12s\n' "max_rss_mb" "$wrk_mb" "$wrkr_mb" "$k6_mb"

    printf '%-28s | %12s | %12s | %12s\n' "rps_ratio wrkr/wrk" "-" "$rps_wrkr_over_wrk" "-"
    printf '%-28s | %12s | %12s | %12s\n' "rps_ratio k6/wrk" "-" "-" "$rps_k6_over_wrk"
    printf '%-28s | %12s | %12s | %12s\n' "rps_ratio wrkr/k6" "-" "$rps_wrkr_over_k6" "-"

    printf '%-28s | %12s | %12s | %12s\n' "mem_ratio wrkr/wrk" "-" "$mem_wrkr_over_wrk" "-"
    printf '%-28s | %12s | %12s | %12s\n' "mem_ratio k6/wrk" "-" "-" "$mem_k6_over_wrk"
    printf '%-28s | %12s | %12s | %12s\n' "mem_ratio wrkr/k6" "-" "$mem_wrkr_over_k6" "-"
  }

  echo
  echo "====================================="
  echo "SUMMARY"
  echo "====================================="
  echo "duration=$DURATION | wrk: threads=$WRK_THREADS conns=$WRK_CONNECTIONS | wrkr: vus=$WRKR_VUS | k6: vus=$K6_VUS"

  print_case_table "GET /hello" "HELLO"
  print_case_table "POST /echo (json + checks)" "POST_JSON"
}

report_and_gate() {
  local ratio_ok="$1"

  WRK_RSS_MB="$(bytes_to_mb "$WRK_RSS_BYTES")"
  WRKR_RSS_MB="$(bytes_to_mb "$WRKR_RSS_BYTES")"

  echo "wrk_max_rss_mb=$WRK_RSS_MB"
  echo "wrkr_max_rss_mb=$WRKR_RSS_MB"

  if [[ "${HAS_K6:-0}" == "1" ]]; then
    K6_RSS_MB="$(bytes_to_mb "$K6_RSS_BYTES")"
    echo "k6_max_rss_mb=$K6_RSS_MB"
  fi

  echo
  echo "wrk_rps=$WRK_RPS"
  echo "wrkr_rps=$WRKR_RPS"

  if [[ "${HAS_K6:-0}" == "1" ]]; then
    echo "k6_rps=$K6_RPS"
  fi

  RPS_RATIO="$(awk -v a="$WRKR_RPS" -v b="$WRK_RPS" 'BEGIN { if ((b + 0.0) <= 0.0) { print "-" } else { printf("%.3f", (a + 0.0) / (b + 0.0)) } }')"
  MEM_RATIO="$(awk -v a="$WRKR_RSS_BYTES" -v b="$WRK_RSS_BYTES" 'BEGIN { if ((b + 0.0) <= 0.0) { print "-" } else { printf("%.3f", (a + 0.0) / (b + 0.0)) } }')"

  echo "rps_ratio_wrkr_over_wrk=$RPS_RATIO"
  echo "mem_ratio_wrkr_over_wrk=$MEM_RATIO"

  if [[ "${HAS_K6:-0}" == "1" ]]; then
    RPS_RATIO_K6_OVER_WRK="$(awk -v a="$K6_RPS" -v b="$WRK_RPS" 'BEGIN { if ((b + 0.0) <= 0.0) { print "-" } else { printf("%.3f", (a + 0.0) / (b + 0.0)) } }')"
    RPS_RATIO_WRKR_OVER_K6="$(awk -v a="$WRKR_RPS" -v b="$K6_RPS" 'BEGIN { if ((b + 0.0) <= 0.0) { print "-" } else { printf("%.3f", (a + 0.0) / (b + 0.0)) } }')"
    MEM_RATIO_K6_OVER_WRK="$(awk -v a="$K6_RSS_BYTES" -v b="$WRK_RSS_BYTES" 'BEGIN { if ((b + 0.0) <= 0.0) { print "-" } else { printf("%.3f", (a + 0.0) / (b + 0.0)) } }')"
    MEM_RATIO_WRKR_OVER_K6="$(awk -v a="$WRKR_RSS_BYTES" -v b="$K6_RSS_BYTES" 'BEGIN { if ((b + 0.0) <= 0.0) { print "-" } else { printf("%.3f", (a + 0.0) / (b + 0.0)) } }')"

    echo "rps_ratio_k6_over_wrk=$RPS_RATIO_K6_OVER_WRK"
    echo "rps_ratio_wrkr_over_k6=$RPS_RATIO_WRKR_OVER_K6"
    echo "mem_ratio_k6_over_wrk=$MEM_RATIO_K6_OVER_WRK"
    echo "mem_ratio_wrkr_over_k6=$MEM_RATIO_WRKR_OVER_K6"
  fi

  OK="$(awk -v wrk="$WRK_RPS" -v wrkr="$WRKR_RPS" -v ratio="$ratio_ok" 'BEGIN { if (wrkr + 0.0 >= (wrk + 0.0) * (ratio + 0.0)) print "yes"; else print "no" }')"
  if [[ "$OK" != "yes" ]]; then
    echo "FAIL: wrkr is too slow vs wrk (ratio_ok=$ratio_ok, actual=$RPS_RATIO)" >&2
    return 0
  fi
  echo "PASS: wrkr_rps >= wrk_rps * $ratio_ok (actual=$RPS_RATIO)"
}

if ! command -v wrk >/dev/null 2>&1; then
  echo "missing required command: wrk" >&2
  echo "Install: macOS: brew install wrk | Ubuntu: sudo apt-get install -y wrk" >&2
  exit 2
fi

HAS_K6=0
if command -v k6 >/dev/null 2>&1; then
  HAS_K6=1
else
  echo "missing required command: k6" >&2
  echo "Install: macOS: brew install k6 | Ubuntu: sudo apt-get install -y k6" >&2
  exit 2
fi

cleanup() {
  if [[ -n "${SERVER_PID:-}" ]]; then
    kill "$SERVER_PID" >/dev/null 2>&1 || true
    wait "$SERVER_PID" >/dev/null 2>&1 || true
  fi
}
trap cleanup EXIT

SERVER_LOG="$(mktemp -t wrkr-testserver.XXXXXX)"

echo "Building binaries..."
RUSTFLAGS_BUILD=""
if [[ "$NATIVE" == "1" ]]; then
  RUSTFLAGS_BUILD="-C target-cpu=native"
fi

(cd "$ROOT_DIR" && RUSTFLAGS="$RUSTFLAGS_BUILD" cargo build -q --release -p wrkr-testserver --bin wrkr-testserver)
(cd "$ROOT_DIR" && RUSTFLAGS="$RUSTFLAGS_BUILD" cargo build -q --release --bin wrkr)

# Start server in background; it prints BASE_URL=http://... as first line.
(
  cd "$ROOT_DIR"
  "$ROOT_DIR/target/release/wrkr-testserver" --bind 127.0.0.1:0
) >"$SERVER_LOG" 2>&1 &
SERVER_PID=$!

BASE_URL=""
for _ in $(seq 1 100); do
  if [[ -s "$SERVER_LOG" ]]; then
    BASE_URL="$(head -n1 "$SERVER_LOG" | sed -n 's/^BASE_URL=//p')"
    if [[ -n "$BASE_URL" ]]; then
      break
    fi
  fi
  sleep 0.05
done

if [[ -z "$BASE_URL" ]]; then
  echo "failed to get BASE_URL from server" >&2
  echo "server log:" >&2
  cat "$SERVER_LOG" >&2
  exit 1
fi

echo "Server: $BASE_URL"

echo
echo "=============================="
echo "CASE: GET /hello (minimal Lua)"
echo "=============================="

echo "== wrk =="
ensure_wrk_cached "HELLO_BENCH" "$ROOT_DIR/tools/perf/wrk_hello.lua" "$BASE_URL"
echo "$WRK_OUT"

echo
echo "== wrkr =="
measure_wrkr "tools/perf/wrkr_hello.lua" "$BASE_URL"
echo "$WRKR_OUT"

if [[ "$HAS_K6" == "1" ]]; then
  echo
  echo "== k6 =="
  ensure_k6_cached "HELLO_BENCH" "$ROOT_DIR/tools/perf/k6_hello.js" "$BASE_URL"
  echo "$K6_OUT"
fi

echo
report_and_gate "$RATIO_OK"
capture_case "HELLO" "$RATIO_OK"

echo
echo "========================================"
echo "CASE: POST /echo (json + response check)"
echo "========================================"

echo "== wrk =="
ensure_wrk_cached "POST_JSON_BENCH" "$ROOT_DIR/tools/perf/wrk_post_json.lua" "$BASE_URL"
echo "$WRK_OUT"

echo
echo "== wrkr =="
measure_wrkr "tools/perf/wrkr_post_json.lua" "$BASE_URL"
echo "$WRKR_OUT"

if [[ "$HAS_K6" == "1" ]]; then
  echo
  echo "== k6 =="
  ensure_k6_cached "POST_JSON_BENCH" "$ROOT_DIR/tools/perf/k6_post_json.js" "$BASE_URL"
  echo "$K6_OUT"
fi

echo
report_and_gate "$RATIO_OK_POST_JSON"
capture_case "POST_JSON" "$RATIO_OK_POST_JSON"

print_summary

if [[ "$FAILURES" -gt 0 ]]; then
  echo
  echo "CHECKS"
  echo "------"
  echo "GET /hello:"
  echo "- wrkr/wrk > $HELLO_GATE_RATIO_OK (actual=$HELLO_RPS_RATIO_WRKR_OVER_WRK) => $HELLO_CHECK_WRKR_OVER_WRK_STATUS"
  echo "- wrkr/k6  > $RATIO_OK_WRKR_OVER_K6 (actual=$HELLO_RPS_RATIO_WRKR_OVER_K6) => $HELLO_CHECK_WRKR_OVER_K6_STATUS"
  echo
  echo "POST /echo (json + checks):"
  echo "- wrkr/wrk > $POST_JSON_GATE_RATIO_OK (actual=$POST_JSON_RPS_RATIO_WRKR_OVER_WRK) => $POST_JSON_CHECK_WRKR_OVER_WRK_STATUS"
  echo "- wrkr/k6  > $RATIO_OK_WRKR_OVER_K6 (actual=$POST_JSON_RPS_RATIO_WRKR_OVER_K6) => $POST_JSON_CHECK_WRKR_OVER_K6_STATUS"
  echo
  echo "OVERALL: FAIL ($FAILURES failing case(s))" >&2
  exit 1
fi

echo
echo "OVERALL: PASS"
