from __future__ import annotations

import json
import re
from dataclasses import dataclass
from typing import Final


class ParseError(RuntimeError):
    """Raised when a tool's output cannot be parsed into the expected metric."""


_DIAG_MAX_CHARS: Final[int] = 4096


@dataclass(frozen=True, slots=True)
class Rps:
    value: float

    def __post_init__(self) -> None:
        if not (self.value >= 0.0):
            raise ValueError(f"RPS must be non-negative, got {self.value!r}")


@dataclass(frozen=True, slots=True)
class ParseDiagnostics:
    kind: str
    message: str
    stdout_tail: str
    stderr_tail: str

    def format(self) -> str:
        out = truncate(self.stdout_tail, _DIAG_MAX_CHARS)
        err = truncate(self.stderr_tail, _DIAG_MAX_CHARS)
        return (
            f"{self.message}\n"
            f"--- {self.kind} stdout (tail) ---\n{out}\n"
            f"--- {self.kind} stderr (tail) ---\n{err}"
        )


def parse_wrk_rps(stdout: str) -> Rps:
    """
    Parse wrk RPS from stdout.

    Expected line format:
        Requests/sec: 12345.67
    """
    for raw in stdout.splitlines():
        line = raw.strip()
        if not line.startswith("Requests/sec:"):
            continue

        rest = line.removeprefix("Requests/sec:").strip()
        token = rest.split()[0] if rest else ""
        if not token:
            raise ParseError("failed to parse wrk RPS (missing token after 'Requests/sec:')")

        try:
            rps = float(token)
        except ValueError as e:
            raise ParseError(f"failed to parse wrk RPS (invalid float token: {token!r})") from e

        return Rps(rps)

    raise ParseError("failed to parse wrk RPS (no 'Requests/sec:' line found)")


def detect_wrk_errors(stdout: str) -> list[str]:
    """Detect correctness issues in wrk output.

    wrk often returns exit code 0 even when there were request failures.

    We treat the following as correctness errors:
    - Non-2xx or 3xx responses > 0
    - Socket errors counts > 0
    """
    errors: list[str] = []

    for raw in stdout.splitlines():
        line = raw.strip()

        if line.startswith("Non-2xx or 3xx responses:"):
            rest = line.removeprefix("Non-2xx or 3xx responses:").strip()
            token = rest.split()[0] if rest else ""
            try:
                n = int(token)
            except ValueError:
                continue
            if n > 0:
                errors.append(f"wrk non-2xx/3xx responses: {n}")

        if line.startswith("Socket errors:"):
            # Example: Socket errors: connect 0, read 12, write 0, timeout 0
            errors.extend(_parse_wrk_socket_errors_line(line))

    return errors


def _parse_wrk_socket_errors_line(line: str) -> list[str]:
    out: list[str] = []
    rest = line.removeprefix("Socket errors:").strip()
    for part in rest.split(","):
        p = part.strip()
        if not p:
            continue
        toks = p.split()
        if len(toks) != 2:
            continue
        kind, n_s = toks[0], toks[1]
        try:
            n = int(n_s)
        except ValueError:
            continue
        if n > 0:
            out.append(f"wrk socket {kind}: {n}")
    return out


def parse_wrkr_rps(*, stdout: str, stderr: str) -> Rps:
    """
    Parse wrkr RPS from stdout/stderr.

    Supported formats (matching legacy + current behavior):
    0) JSON progress lines (NDJSON) emitted by `wrkr --output json`:
        {"elapsed_secs": 1, "total_requests": 123, "req_per_sec_avg": 123.0, ...}
    1) Legacy:
        rps: 1234
    2) k6-like summary lines:
        http_reqs.......................: 1085653 (217130.60000/s)
        grpc_reqs.......................: 123      (456.789/s)
        iterations......................: ...
       Preference order: grpc_reqs, then http_reqs, then iterations.
    """
    # Prefer machine-readable NDJSON when present.
    js = try_parse_wrkr_json_summary(stdout=stdout, stderr=stderr)
    if js is not None:
        return Rps(js.rps)

    try:
        return _parse_wrkr_rps_text(stdout)
    except ParseError:
        try:
            return _parse_wrkr_rps_text(stderr)
        except ParseError as e:
            diag = ParseDiagnostics(
                kind="wrkr",
                message=str(e),
                stdout_tail=tail_lines(stdout, 12),
                stderr_tail=tail_lines(stderr, 12),
            )
            raise ParseError(diag.format()) from e


def _parse_wrkr_rps_text(text: str) -> Rps:
    # Legacy: rps: 1234
    for raw in text.splitlines():
        line = raw.strip()
        if not line.startswith("rps:"):
            continue

        rest = line.removeprefix("rps:").strip()
        token = rest.split()[0] if rest else ""
        if not token:
            raise ParseError("failed to parse wrkr RPS (missing token after 'rps:')")

        try:
            rps = float(token)
        except ValueError as e:
            raise ParseError(f"failed to parse wrkr RPS (invalid float token: {token!r})") from e

        return Rps(rps)

    # k6-like summary: prefer grpc_reqs (grpc scripts may also print http_reqs=0).
    grpc_rps: float | None = None
    http_rps: float | None = None
    iterations_rps: float | None = None

    for line in text.splitlines():
        if "grpc_reqs" in line:
            r = parse_paren_rate_token(line)
            if r is not None:
                grpc_rps = r
        elif "http_reqs" in line:
            r = parse_paren_rate_token(line)
            if r is not None:
                http_rps = r
        elif "iterations" in line:
            r = parse_paren_rate_token(line)
            if r is not None:
                iterations_rps = r

    if grpc_rps is not None:
        return Rps(grpc_rps)
    if http_rps is not None:
        return Rps(http_rps)
    if iterations_rps is not None:
        return Rps(iterations_rps)

    raise ParseError("failed to parse wrkr RPS")


@dataclass(frozen=True, slots=True)
class WrkrJsonSummary:
        """Summary extracted from wrkr NDJSON output.

        `wrkr --output json` emits:
            - `kind="progress"` lines during the run
            - a final `kind="summary"` line at the end

        We combine the last progress line (for rates) with the final summary line
        (for totals and latency percentiles where available).
        """

        elapsed_secs: int
        total_requests: int
        rps: float

        checks_failed_total: int
        latency_mean_ms: float | None
        latency_p50_ms: int | None
        latency_p90_ms: int | None
        latency_p99_ms: int | None
        latency_max_ms: int | None

        bytes_received_per_sec: int | None
        bytes_sent_per_sec: int | None


def try_parse_wrkr_json_summary(*, stdout: str, stderr: str) -> WrkrJsonSummary | None:
    """Try to parse wrkr's JSON progress output (NDJSON).

    Returns None if no JSON progress lines are found.

    Raises ParseError if JSON is detected but required fields are missing/invalid.
    """

    last_progress: dict[str, object] | None = None
    last_summary: dict[str, object] | None = None
    saw_json = False

    for text in (stdout, stderr):
        for raw in text.splitlines():
            obj = _try_parse_json_object_line(raw)
            if obj is None:
                continue
            saw_json = True

            kind = obj.get("kind")
            if kind == "progress" or (
                kind is None and "elapsed_secs" in obj and "total_requests" in obj
            ):
                last_progress = obj
            elif kind == "summary" or (
                kind is None and "totals" in obj and "scenarios" in obj
            ):
                last_summary = obj

    if last_progress is None and last_summary is None:
        return None if not saw_json else _raise_wrkr_json_error(stdout=stdout, stderr=stderr)

    return _parse_wrkr_json_summary_obj(
        progress=last_progress,
        summary=last_summary,
        stdout=stdout,
        stderr=stderr,
    )


def _try_parse_json_object_line(line: str) -> dict[str, object] | None:
    s = line.strip()
    if not s or not s.startswith("{"):
        return None
    try:
        v = json.loads(s)
    except Exception:
        return None
    if not isinstance(v, dict):
        return None
    # Best-effort: assume string keys.
    return v  # type: ignore[return-value]


def _parse_wrkr_json_summary_obj(
    *,
    progress: dict[str, object] | None,
    summary: dict[str, object] | None,
    stdout: str,
    stderr: str,
) -> WrkrJsonSummary:
    try:
        # ---- Rates from progress ----
        elapsed_secs: int
        rps_avg: float
        bytes_received_per_sec: int | None
        bytes_sent_per_sec: int | None

        if progress is None:
            # We cannot compute RPS from summary alone (no rates there).
            raise ParseError("wrkr json: missing progress line (kind=progress)")

        elapsed_secs = _json_int(progress, "elapsed_secs")

        # Prefer explicit average if present.
        rps_avg_opt = _json_float(progress, "req_per_sec_avg", default=None)
        if rps_avg_opt is not None:
            rps_avg = rps_avg_opt
        else:
            # Derive from totals if we can, otherwise fall back to instantaneous rps.
            total_requests_progress = _json_int(progress, "total_requests")
            if elapsed_secs > 0:
                rps_avg = float(total_requests_progress) / float(elapsed_secs)
            else:
                rps_avg = _json_float(progress, "requests_per_sec") or 0.0

        bytes_received_per_sec = _json_int(progress, "bytes_received_per_sec")
        bytes_sent_per_sec = _json_int(progress, "bytes_sent_per_sec")

        # ---- Totals & percentiles: prefer final summary ----
        total_requests: int
        checks_failed_total: int
        latency_mean_ms: float | None
        latency_p50_ms: int | None
        latency_p90_ms: int | None
        latency_p99_ms: int | None
        latency_max_ms: int | None

        if summary is not None:
            totals = _json_obj(summary, "totals")
            total_requests = _json_int(totals, "requests_total")
            checks_failed_total = _json_int(totals, "checks_failed_total")

            # Best-effort: use the first scenario latency if present.
            scenarios = _json_list(summary, "scenarios")
            latency = None
            if scenarios:
                first = scenarios[0]
                if isinstance(first, dict):
                    lat = first.get("latency_ms")
                    if isinstance(lat, dict):
                        latency = lat

            latency_mean_ms = _json_float(latency, "mean", default=None) if latency else None
            latency_p50_ms = (
                _ms_round_int(_json_float(latency, "p50", default=None))
                if latency
                else None
            )
            latency_p90_ms = (
                _ms_round_int(_json_float(latency, "p90", default=None))
                if latency
                else None
            )
            latency_p99_ms = (
                _ms_round_int(_json_float(latency, "p99", default=None))
                if latency
                else None
            )
            latency_max_ms = (
                _ms_round_int(_json_float(latency, "max", default=None))
                if latency
                else None
            )
        else:
            # Back-compat: derive totals from progress line.
            total_requests = _json_int(progress, "total_requests")
            checks_failed_total = _json_int(progress, "checks_failed_total")

            latency_mean_ms = _json_float(progress, "latency_mean", default=None)
            latency_p50_ms = _json_int(progress, "latency_p50")
            latency_p90_ms = _json_int(progress, "latency_p90")
            latency_p99_ms = _json_int(progress, "latency_p99")
            latency_max_ms = _json_int(progress, "latency_max")
    except ParseError as e:
        diag = ParseDiagnostics(
            kind="wrkr-json",
            message=str(e),
            stdout_tail=tail_lines(stdout, 12),
            stderr_tail=tail_lines(stderr, 12),
        )
        raise ParseError(diag.format()) from e

    if rps_avg < 0.0:
        raise ParseError(f"wrkr json: rps must be non-negative, got {rps_avg!r}")

    return WrkrJsonSummary(
        elapsed_secs=elapsed_secs,
        total_requests=total_requests,
        rps=float(rps_avg),
        checks_failed_total=checks_failed_total,
        latency_mean_ms=None if latency_mean_ms is None else float(latency_mean_ms),
        latency_p50_ms=latency_p50_ms,
        latency_p90_ms=latency_p90_ms,
        latency_p99_ms=latency_p99_ms,
        latency_max_ms=latency_max_ms,
        bytes_received_per_sec=bytes_received_per_sec,
        bytes_sent_per_sec=bytes_sent_per_sec,
    )


def _raise_wrkr_json_error(*, stdout: str, stderr: str) -> None:
    diag = ParseDiagnostics(
        kind="wrkr-json",
        message="failed to parse wrkr JSON progress lines (no progress objects found)",
        stdout_tail=tail_lines(stdout, 12),
        stderr_tail=tail_lines(stderr, 12),
    )
    raise ParseError(diag.format())


def _json_int(obj: dict[str, object], key: str) -> int:
    if key not in obj:
        raise ParseError(f"wrkr json: missing key {key!r}")
    v = obj[key]
    if isinstance(v, bool):
        raise ParseError(f"wrkr json: key {key!r} must be an int, got bool")
    if isinstance(v, int):
        return v
    if isinstance(v, float) and v.is_integer():
        return int(v)
    raise ParseError(f"wrkr json: key {key!r} must be an int, got {type(v).__name__}")


def _json_float(
    obj: dict[str, object],
    key: str,
    *,
    default: float | None = ...,
) -> float | None:
    if key not in obj:
        if default is ...:
            raise ParseError(f"wrkr json: missing key {key!r}")
        return default
    v = obj[key]
    if isinstance(v, bool):
        raise ParseError(f"wrkr json: key {key!r} must be a float, got bool")
    if isinstance(v, (int, float)):
        return float(v)
    raise ParseError(f"wrkr json: key {key!r} must be a float, got {type(v).__name__}")


def _json_obj(parent: dict[str, object], key: str) -> dict[str, object]:
    if key not in parent:
        raise ParseError(f"wrkr json: missing key {key!r}")
    v = parent[key]
    if not isinstance(v, dict):
        raise ParseError(f"wrkr json: key {key!r} must be an object")
    return v  # type: ignore[return-value]


def _json_list(parent: dict[str, object], key: str) -> list[object]:
    if key not in parent:
        raise ParseError(f"wrkr json: missing key {key!r}")
    v = parent[key]
    if not isinstance(v, list):
        raise ParseError(f"wrkr json: key {key!r} must be a list")
    return v


def _ms_round_int(v: float | None) -> int | None:
    if v is None:
        return None
    if v < 0.0:
        return None
    return int(round(v))


def parse_k6_http_rps(*, stdout: str, stderr: str) -> Rps:
    """
    Parse k6 HTTP RPS from stdout/stderr.

    Preferred:
        http_reqs ...: ... 1234.5/s
    Fallback (our scripts are 1 request per iteration):
        iterations ...: ... 1234.5/s
    Last resort:
        running (02.0s), ... 155325 complete ... iterations
      => completed / seconds
    """
    try:
        return _parse_k6_http_rps_text(stdout)
    except ParseError:
        try:
            return _parse_k6_http_rps_text(stderr)
        except ParseError as e:
            diag = ParseDiagnostics(
                kind="k6",
                message=str(e),
                stdout_tail=tail_lines(stdout, 12),
                stderr_tail=tail_lines(stderr, 12),
            )
            raise ParseError(diag.format()) from e


def _parse_k6_http_rps_text(text: str) -> Rps:
    for line in text.splitlines():
        if "http_reqs" in line:
            rate = parse_slash_s_token(line)
            if rate is not None:
                return Rps(rate)

    for line in text.splitlines():
        if "iterations" in line:
            rate = parse_slash_s_token(line)
            if rate is not None:
                return Rps(rate)

    for line in text.splitlines():
        rate = parse_k6_progress_rps(line)
        if rate is not None:
            return Rps(rate)

    raise ParseError("failed to parse k6 http RPS")


def parse_k6_grpc_rps(*, stdout: str, stderr: str) -> Rps:
    """
    Parse k6 gRPC RPS from stdout/stderr.

    Accepts grpc_reqs and iterations lines containing a token like:
      1234.5/s
    Some k6 builds may only print http_reqs; we fall back to HTTP parsing.
    """
    try:
        return _parse_k6_grpc_rps_text(stdout)
    except ParseError:
        try:
            return _parse_k6_grpc_rps_text(stderr)
        except ParseError as e:
            diag = ParseDiagnostics(
                kind="k6",
                message=str(e),
                stdout_tail=tail_lines(stdout, 12),
                stderr_tail=tail_lines(stderr, 12),
            )
            raise ParseError(diag.format()) from e


def _parse_k6_grpc_rps_text(text: str) -> Rps:
    for line in text.splitlines():
        if "grpc_reqs" not in line and "iterations" not in line:
            continue
        rate = parse_slash_s_token(line)
        if rate is not None:
            return Rps(rate)

    # Fallback: some k6 builds print only http_reqs.
    return _parse_k6_http_rps_text(text)


_K6_PERCENT_RE: Final[re.Pattern[str]] = re.compile(r"(?P<pct>[0-9]+(?:\.[0-9]+)?)%")
_K6_REQUEST_FAILED_RE: Final[re.Pattern[str]] = re.compile(r'msg="Request Failed"')


def parse_k6_http_req_failed_rate(*, stdout: str, stderr: str) -> float | None:
    """Parse k6 http_req_failed percentage as a fraction in [0, 1]."""
    return _parse_k6_req_failed_rate(metric="http_req_failed", stdout=stdout, stderr=stderr)


def parse_k6_grpc_req_failed_rate(*, stdout: str, stderr: str) -> float | None:
    """Parse k6 grpc_req_failed percentage as a fraction in [0, 1].

    Falls back to http_req_failed for builds/scripts that don't emit grpc_req_failed.
    """
    rate = _parse_k6_req_failed_rate(metric="grpc_req_failed", stdout=stdout, stderr=stderr)
    if rate is not None:
        return rate
    return _parse_k6_req_failed_rate(metric="http_req_failed", stdout=stdout, stderr=stderr)


def _parse_k6_req_failed_rate(*, metric: str, stdout: str, stderr: str) -> float | None:
    # Prefer stdout, then stderr.
    v = _parse_k6_req_failed_rate_text(metric=metric, text=stdout)
    if v is not None:
        return v
    return _parse_k6_req_failed_rate_text(metric=metric, text=stderr)


def _parse_k6_req_failed_rate_text(*, metric: str, text: str) -> float | None:
    # Typical line:
    #   http_req_failed..............: 0.15% ✓ 123 ✗ 4
    #   grpc_req_failed..............: 0.00% ✓ ...
    for raw in text.splitlines():
        line = raw.strip()
        if metric not in line:
            continue

        m = _K6_PERCENT_RE.search(line)
        if m is None:
            return None
        try:
            pct = float(m.group("pct"))
        except ValueError:
            return None

        if pct < 0.0:
            return None
        return pct / 100.0

    return None


def count_k6_request_failed_warnings(*, stdout: str, stderr: str) -> int:
    """Count occurrences of k6's `msg="Request Failed"` warnings."""
    return _count_re(_K6_REQUEST_FAILED_RE, stdout) + _count_re(_K6_REQUEST_FAILED_RE, stderr)


def _count_re(pat: re.Pattern[str], text: str) -> int:
    return sum(1 for _ in pat.finditer(text))


def parse_paren_rate_token(line: str) -> float | None:
    """
    Parse a parenthesized rate token like:
      http_reqs...: 1085653 (217130.60000/s)

    Returns the numeric rate (per second), or None if not found.
    """
    try:
        start = line.index("(")
        end = line.index(")", start + 1)
    except ValueError:
        return None

    inside = line[start + 1 : end].strip()
    if not inside.endswith("/s"):
        return None

    number = inside.removesuffix("/s").strip()
    try:
        return float(number)
    except ValueError:
        return None


def parse_slash_s_token(line: str) -> float | None:
    """
    Find a token matching `.../s` in whitespace-split tokens.

    Handles tokens like:
      217130.60/s
      (217130.60000/s)
      12.3k/s  (some builds)
    """
    for raw in line.split():
        token = raw.strip("(),")
        if not token.endswith("/s"):
            continue

        number = token.removesuffix("/s")
        v = parse_si_float(number)
        if v is not None:
            return v

    return None


def parse_si_float(token: str) -> float | None:
    """
    Parse a float that may have SI suffixes:
      123.4, 123.4k, 1.2M, 3.4G
    """
    t = token.strip()
    if not t:
        return None

    last = t[-1]
    if last in {"k", "K"}:
        num, mul = t[:-1], 1_000.0
    elif last in {"m", "M"}:
        num, mul = t[:-1], 1_000_000.0
    elif last in {"g", "G"}:
        num, mul = t[:-1], 1_000_000_000.0
    else:
        num, mul = t, 1.0

    try:
        return float(num) * mul
    except ValueError:
        return None


def parse_k6_progress_rps(line: str) -> float | None:
    """
    Parse a k6 progress line and compute RPS as completed/seconds.

    Example:
      running (02.0s), 000/256 VUs, 155325 complete and 0 interrupted iterations
    """
    s = line.strip()
    if not s.startswith("running ("):
        return None
    if " complete" not in s or "iterations" not in s:
        return None

    seconds = parse_k6_running_seconds(s)
    if seconds is None or seconds <= 0.0:
        return None

    completed = parse_k6_completed_iterations(s)
    if completed is None:
        return None

    return float(completed) / seconds


def parse_k6_running_seconds(line: str) -> float | None:
    try:
        rest = line.removeprefix("running (")
        inside = rest[: rest.index(")")]
    except ValueError:
        return None

    inside = inside.strip()
    if not inside.endswith("s"):
        return None

    number = inside.removesuffix("s")
    try:
        return float(number)
    except ValueError:
        return None


def parse_k6_completed_iterations(line: str) -> int | None:
    """
    Find the token immediately before the word 'complete' and parse it as an int.

    Handles commas in the number.
    """
    prev: str | None = None
    for tok in line.split():
        if tok == "complete":
            if prev is None:
                return None
            n = prev.rstrip(",").replace(",", "")
            try:
                return int(n)
            except ValueError:
                return None
        prev = tok
    return None


def tail_lines(text: str, n: int) -> str:
    if n <= 0:
        return ""
    lines = text.splitlines()
    if len(lines) <= n:
        return text
    return "\n".join(lines[-n:])


def truncate(text: str, max_chars: int) -> str:
    if max_chars <= 0:
        return ""
    if len(text) <= max_chars:
        return text
    return text[:max_chars] + "..."
