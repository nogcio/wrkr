from __future__ import annotations

import json
import os
from dataclasses import dataclass
from pathlib import Path

from .config import Config, parse_duration_to_seconds
from .exec import RunResult, run_with_peak_rss_sampling_streaming
from .parse import (
    ParseError,
    Rps,
    count_k6_request_failed_warnings,
    detect_wrk_errors,
    parse_k6_grpc_req_failed_rate,
    parse_k6_grpc_rps,
    parse_k6_http_req_failed_rate,
    parse_k6_http_rps,
    parse_wrk_rps,
    parse_wrkr_rps,
    try_parse_wrkr_json_summary,
)
from .report import format_grpc_summary_line, format_http_summary_line
from .tool_detection import ToolPaths
from .ui import RunUI


def _format_wrkr_json_progress_line_for_ui(line: str) -> str | None:
    s = line.strip()
    if not s.startswith("{"):
        return None
    try:
        obj = json.loads(s)
    except Exception:
        return None
    if not isinstance(obj, dict):
        return None

    # Heuristic: only format wrkr NDJSON progress lines.
    if obj.get("kind") not in {None, "progress"}:
        return None
    if "elapsed_secs" not in obj or "total_requests" not in obj:
        return None

    try:
        t = int(obj.get("elapsed_secs"))
        total = int(obj.get("total_requests"))
        rps_avg = obj.get("req_per_sec_avg")
        rps = float(rps_avg) if rps_avg is not None else float(obj.get("requests_per_sec"))

        p99 = int(obj.get("latency_p99"))
        mean = float(obj.get("latency_mean"))
        failed = int(obj.get("checks_failed_total"))
    except Exception:
        return None

    return f"wrkr: t={t:>3}s rps_avg={rps:>10.3f} p99={p99:>4}ms mean={mean:>7.3f}ms failed_checks={failed} total={total}"


def _fmt_ms_i(v: int | None) -> str:
    return "-" if v is None else f"{v}"


def _fmt_ms_f(v: float | None) -> str:
    return "-" if v is None else f"{v:.3f}"


def _fmt_int(v: int | None) -> str:
    return "-" if v is None else f"{v}"


def _no_proxy_env_for_localhost() -> dict[str, str]:
    # Many developer environments set HTTP(S)_PROXY; ensure we never proxy local testserver traffic.
    # Both reqwest (wrkr) and Go net/http (k6) respect NO_PROXY/no_proxy.
    add = ["127.0.0.1", "localhost", "::1"]

    def merge(existing: str | None) -> str:
        parts: list[str] = []
        if existing:
            parts.extend([p.strip() for p in existing.split(",") if p.strip()])
        for p in add:
            if p not in parts:
                parts.append(p)
        return ",".join(parts)

    merged = merge(os.environ.get("NO_PROXY") or os.environ.get("no_proxy"))
    return {"NO_PROXY": merged, "no_proxy": merged}


class CaseError(RuntimeError):
    """Raised when a case cannot be executed as designed (misconfiguration, missing scripts, etc.)."""


@dataclass(frozen=True, slots=True)
class HttpCaseScripts:
    wrk: str
    wrkr: str
    k6: str


@dataclass(frozen=True, slots=True)
class HttpCase:
    title: str
    scripts: HttpCaseScripts
    ratio_ok_wrkr_over_wrk: float
    ratio_ok_wrkr_over_k6: float


@dataclass(frozen=True, slots=True)
class GrpcCaseScripts:
    wrkr: str
    k6: str


@dataclass(frozen=True, slots=True)
class GrpcCase:
    title: str
    scripts: GrpcCaseScripts
    ratio_ok_wrkr_over_k6: float


@dataclass(frozen=True, slots=True)
class HttpCaseOutcome:
    failures: int
    wrk_rps: Rps | None
    failure_messages: tuple[str, ...]
    summary_lines: tuple[str, ...]


@dataclass(frozen=True, slots=True)
class GrpcCaseOutcome:
    failures: int
    wrkr_rps: Rps | None
    failure_messages: tuple[str, ...]
    summary_lines: tuple[str, ...]


def default_http_cases(cfg: Config) -> list[HttpCase]:
    """
    Default HTTP cases.

    The script paths are relative to the wrkr repo root and refer to files under `tools/perf/`.
    """
    return [
        HttpCase(
            title="GET /hello",
            scripts=HttpCaseScripts(
                wrk="tools/perf/wrk_hello.lua",
                wrkr="tools/perf/wrkr_hello.lua",
                k6="tools/perf/k6_hello.js",
            ),
            ratio_ok_wrkr_over_wrk=cfg.ratios.ratio_ok_get_hello,
            ratio_ok_wrkr_over_k6=cfg.ratios.ratio_ok_wrkr_over_k6,
        ),
        HttpCase(
            title="POST /echo (json + checks)",
            scripts=HttpCaseScripts(
                wrk="tools/perf/wrk_post_json.lua",
                wrkr="tools/perf/wrkr_post_json.lua",
                k6="tools/perf/k6_post_json.js",
            ),
            ratio_ok_wrkr_over_wrk=cfg.ratios.ratio_ok_post_json,
            ratio_ok_wrkr_over_k6=cfg.ratios.ratio_ok_wrkr_over_k6,
        ),
        HttpCase(
            title="POST /analytics/aggregate (wfb json + checks)",
            scripts=HttpCaseScripts(
                wrk="tools/perf/wrk_wfb_json_aggregate.lua",
                wrkr="tools/perf/wrkr_wfb_json_aggregate.lua",
                k6="tools/perf/k6_wfb_json_aggregate.js",
            ),
            ratio_ok_wrkr_over_wrk=cfg.ratios.ratio_ok_wfb_json_aggregate,
            ratio_ok_wrkr_over_k6=cfg.ratios.ratio_ok_wrkr_over_k6,
        ),
    ]


def default_grpc_cases(cfg: Config) -> list[GrpcCase]:
    """
    Default gRPC cases.

    The script paths are relative to the wrkr repo root and refer to files under `tools/perf/`.
    """
    return [
        GrpcCase(
            title="gRPC Echo (plaintext)",
            scripts=GrpcCaseScripts(
                wrkr="tools/perf/wrkr_grpc_plaintext.lua",
                k6="tools/perf/k6_grpc_plaintext.js",
            ),
            ratio_ok_wrkr_over_k6=cfg.ratios.ratio_ok_grpc_wrkr_over_k6,
        ),
        GrpcCase(
            title="gRPC AggregateOrders (wfb)",
            scripts=GrpcCaseScripts(
                wrkr="tools/perf/wfb_grpc_aggregate.lua",
                k6="tools/perf/k6_wfb_grpc_aggregate.js",
            ),
            ratio_ok_wrkr_over_k6=cfg.ratios.ratio_ok_wfb_grpc_aggregate_wrkr_over_k6,
        ),
    ]


def run_http_case(
    *, cfg: Config, tools: ToolPaths, base_url: str, case: HttpCase, ui: RunUI
) -> HttpCaseOutcome:
    title = case.title
    scripts = case.scripts

    ui.log(f"CASE: {title}")

    _ensure_script_exists(cfg.root, scripts.wrkr)
    _ensure_script_exists(cfg.root, scripts.wrk)
    _ensure_script_exists(cfg.root, scripts.k6)

    failures = 0
    failure_messages: list[str] = []
    summary_lines: list[str] = []

    wrk_res: RunResult | None
    wrk_ok = True
    if tools.wrk is not None:
        wrk_argv = [
            str(tools.wrk),
            f"-t{cfg.tuning.wrk_threads}",
            f"-c{cfg.tuning.wrk_connections}",
            f"-d{cfg.tuning.duration}",
            "-s",
            str(cfg.root / scripts.wrk),
            base_url,
        ]
        with ui.step(f"{title}: wrk"):
            ui.set_current_command(label="wrk", argv=wrk_argv, cwd=cfg.root, env=None)
            wrk_res = run_with_peak_rss_sampling_streaming(
                wrk_argv,
                cwd=cfg.root,
                on_stdout_line=ui.tail,
                on_stderr_line=lambda line: ui.tail(line, style="dim"),
            )

        if wrk_res.returncode != 0:
            wrk_ok = False
            msg = f"FAIL: wrk exited with code {wrk_res.returncode}"
            ui.log(msg, style="red")
            failure_messages.append(msg)
            failures += 1
        else:
            wrk_errs = detect_wrk_errors(wrk_res.stdout)
            if wrk_errs:
                wrk_ok = False
                for e in wrk_errs:
                    msg = f"FAIL: {e}"
                    ui.log(msg, style="red")
                    failure_messages.append(msg)
                    failures += 1
    else:
        ui.log("wrk: skipped (not installed)")
        wrk_res = None
        wrk_ok = False

    ui.log("wrkr")
    wrkr_env = {"BASE_URL": base_url, **_no_proxy_env_for_localhost()}
    wrkr_argv = [
        str(tools.wrkr),
        "run",
        scripts.wrkr,
        "--output",
        "json",
        "--duration",
        cfg.tuning.duration,
        "--vus",
        str(cfg.tuning.wrkr_vus),
        "--env",
        f"BASE_URL={base_url}",
    ]
    with ui.step(f"{title}: wrkr"):
        ui.set_current_command(label="wrkr", argv=wrkr_argv, cwd=cfg.root, env=wrkr_env)
        wrkr_res = run_with_peak_rss_sampling_streaming(
            wrkr_argv,
            cwd=cfg.root,
            env=wrkr_env,
            on_stdout_line=lambda line: ui.tail(
                _format_wrkr_json_progress_line_for_ui(line) or line
            ),
            on_stderr_line=lambda line: ui.tail(line, style="dim"),
        )

    wrkr_ok = True
    if wrkr_res.returncode != 0:
        wrkr_ok = False
        msg = f"FAIL: wrkr exited with code {wrkr_res.returncode}"
        ui.log(msg, style="red")
        failure_messages.append(msg)
        failures += 1

    k6_res: RunResult | None
    k6_ok = True
    if tools.k6 is not None:
        k6_argv = [
            str(tools.k6),
            "run",
            "--vus",
            str(cfg.effective_k6_vus()),
            "--duration",
            cfg.tuning.duration,
            str(cfg.root / scripts.k6),
        ]
        with ui.step(f"{title}: k6"):
            ui.set_current_command(label="k6", argv=k6_argv, cwd=cfg.root, env=wrkr_env)
            k6_res = run_with_peak_rss_sampling_streaming(
                k6_argv,
                cwd=cfg.root,
                env=wrkr_env,
                on_stdout_line=ui.tail,
                on_stderr_line=lambda line: ui.tail(line, style="dim"),
            )

        if k6_res.returncode != 0:
            k6_ok = False
            msg = f"FAIL: k6 exited with code {k6_res.returncode}"
            ui.log(msg, style="red")
            failure_messages.append(msg)
            failures += 1
    else:
        ui.log("k6: skipped (not installed)")
        k6_res = None
        k6_ok = False

    wrkr_rps: Rps | None
    wrkr_json = try_parse_wrkr_json_summary(stdout=wrkr_res.stdout, stderr=wrkr_res.stderr)
    try:
        wrkr_rps = parse_wrkr_rps(
            stdout=wrkr_res.stdout,
            stderr=wrkr_res.stderr,
            test_duration_seconds=parse_duration_to_seconds(cfg.tuning.duration),
        )
    except ParseError as e:
        wrkr_rps = None
        wrkr_ok = False
        msg = f"FAIL: could not parse wrkr RPS ({e})"
        ui.log(msg, style="red")
        failure_messages.append(msg)
        failures += 1

    if wrkr_json is not None and wrkr_json.checks_failed_total > 0:
        wrkr_ok = False
        msg = f"FAIL: wrkr has failed checks (count={wrkr_json.checks_failed_total})"
        ui.log(msg, style="red")
        failure_messages.append(msg)
        failures += 1

    wrk_rps: Rps | None = None
    if wrk_res is not None:
        try:
            wrk_rps = parse_wrk_rps(wrk_res.stdout)
        except ParseError as e:
            wrk_rps = None
            wrk_ok = False
            msg = f"FAIL: could not parse wrk RPS ({e})"
            ui.log(msg, style="red")
            failure_messages.append(msg)
            failures += 1

    k6_rps: Rps | None = None
    if k6_res is not None:
        try:
            k6_rps = parse_k6_http_rps(stdout=k6_res.stdout, stderr=k6_res.stderr)
        except ParseError as e:
            k6_rps = None
            k6_ok = False
            msg = f"FAIL: could not parse k6 RPS ({e})"
            ui.log(msg, style="red")
            failure_messages.append(msg)
            failures += 1

        k6_failed = parse_k6_http_req_failed_rate(stdout=k6_res.stdout, stderr=k6_res.stderr)
        if k6_failed is not None and k6_failed > 0.0:
            k6_ok = False
            msg = f"FAIL: k6 has request failures (http_req_failed={k6_failed:.3%})"
            ui.log(msg, style="red")
            failure_messages.append(msg)
            failures += 1

        warn_n = count_k6_request_failed_warnings(stdout=k6_res.stdout, stderr=k6_res.stderr)
        if warn_n > 0:
            k6_ok = False
            msg = f"FAIL: k6 emitted Request Failed warnings (count={warn_n})"
            ui.log(msg, style="red")
            failure_messages.append(msg)
            failures += 1

    ui.log(
        format_http_summary_line(
            wrk_res=wrk_res,
            wrkr_res=wrkr_res,
            k6_res=k6_res,
            wrk_rps=wrk_rps,
            wrkr_rps=wrkr_rps,
            k6_rps=k6_rps,
        )
    )

    summary_lines.append(f"HTTP {title}")
    summary_lines.append(
        f"  scripts: wrk={scripts.wrk} wrkr={scripts.wrkr} k6={scripts.k6} duration={cfg.tuning.duration}"
    )
    summary_lines.append(
        f"  wrk : {'OK' if (tools.wrk is not None and wrk_ok) else ('SKIP' if tools.wrk is None else 'FAIL')} rps={wrk_rps.value:.3f}"
        if wrk_rps is not None
        else f"  wrk : {'OK' if (tools.wrk is not None and wrk_ok) else ('SKIP' if tools.wrk is None else 'FAIL')} rps=-"
    )
    summary_lines.append(
        f"  wrkr: {'OK' if wrkr_ok else 'FAIL'} rps={wrkr_rps.value:.3f}"
        if wrkr_rps is not None
        else f"  wrkr: {'OK' if wrkr_ok else 'FAIL'} rps=-"
    )
    if wrkr_json is not None:
        summary_lines.append(
            "  wrkr json: "
            f"p50={_fmt_ms_i(wrkr_json.latency_p50_ms)}ms p90={_fmt_ms_i(wrkr_json.latency_p90_ms)}ms "
            f"p99={_fmt_ms_i(wrkr_json.latency_p99_ms)}ms max={_fmt_ms_i(wrkr_json.latency_max_ms)}ms "
            f"mean={_fmt_ms_f(wrkr_json.latency_mean_ms)}ms failed_checks={wrkr_json.checks_failed_total} "
            f"rx/s={_fmt_int(wrkr_json.bytes_received_per_sec)} tx/s={_fmt_int(wrkr_json.bytes_sent_per_sec)}"
        )
    if tools.k6 is None:
        summary_lines.append("  k6  : SKIP")
    else:
        summary_lines.append(
            f"  k6  : {'OK' if k6_ok else 'FAIL'} rps={k6_rps.value:.3f}"
            if k6_rps is not None
            else f"  k6  : {'OK' if k6_ok else 'FAIL'} rps=-"
        )

    # Gate: wrkr vs wrk (inclusive)
    if wrk_rps is not None and wrkr_rps is not None and wrk_ok and wrkr_ok:
        ratio_actual = wrkr_rps.value / wrk_rps.value if wrk_rps.value > 0 else float("inf")
        if is_too_slow(
            wrkr=wrkr_rps, other=wrk_rps, ratio=case.ratio_ok_wrkr_over_wrk, inclusive=True
        ):
            msg = (
                "FAIL: wrkr is too slow vs wrk "
                f"(ratio_ok={case.ratio_ok_wrkr_over_wrk}, ratio_actual={ratio_actual:.3f})"
            )
            ui.log(msg, style="red")
            failure_messages.append(msg)
            failures += 1
        else:
            msg = (
                f"PASS: wrkr/wrk >= {case.ratio_ok_wrkr_over_wrk} (ratio_actual={ratio_actual:.3f})"
            )
            ui.log(msg)
        summary_lines.append(
            f"  gate wrkr/wrk: ratio_ok={case.ratio_ok_wrkr_over_wrk} ratio_actual={ratio_actual:.3f}"
        )
    else:
        summary_lines.append("  gate wrkr/wrk: SKIP (correctness failed or missing tool)")

    # Gate: wrkr vs k6 (strict)
    if k6_rps is not None and wrkr_rps is not None and k6_ok and wrkr_ok:
        ratio_actual = wrkr_rps.value / k6_rps.value if k6_rps.value > 0 else float("inf")
        if is_too_slow(
            wrkr=wrkr_rps, other=k6_rps, ratio=case.ratio_ok_wrkr_over_k6, inclusive=False
        ):
            msg = (
                "FAIL: wrkr is too slow vs k6 "
                f"(ratio_ok={case.ratio_ok_wrkr_over_k6}, ratio_actual={ratio_actual:.3f})"
            )
            ui.log(msg, style="red")
            failure_messages.append(msg)
            failures += 1
        else:
            msg = f"PASS: wrkr/k6 > {case.ratio_ok_wrkr_over_k6} (ratio_actual={ratio_actual:.3f})"
            ui.log(msg)
        summary_lines.append(
            f"  gate wrkr/k6 : ratio_ok={case.ratio_ok_wrkr_over_k6} ratio_actual={ratio_actual:.3f}"
        )
    else:
        summary_lines.append("  gate wrkr/k6 : SKIP (correctness failed or missing tool)")

    return HttpCaseOutcome(
        failures=failures,
        wrk_rps=wrk_rps,
        failure_messages=tuple(failure_messages),
        summary_lines=tuple(summary_lines),
    )


def run_grpc_case(
    *,
    cfg: Config,
    tools: ToolPaths,
    grpc_target: str,
    case: GrpcCase,
    ui: RunUI,
) -> GrpcCaseOutcome:
    title = case.title
    scripts = case.scripts

    ui.log(f"CASE: {title}")

    _ensure_script_exists(cfg.root, scripts.wrkr)
    _ensure_script_exists(cfg.root, scripts.k6)

    failures = 0
    failure_messages: list[str] = []
    summary_lines: list[str] = []

    ui.log("wrkr")
    wrkr_env = {"GRPC_TARGET": grpc_target, **_no_proxy_env_for_localhost()}
    wrkr_argv = [
        str(tools.wrkr),
        "run",
        scripts.wrkr,
        "--output",
        "json",
        "--duration",
        cfg.tuning.duration,
        "--vus",
        str(cfg.tuning.wrkr_vus),
        "--env",
        f"GRPC_TARGET={grpc_target}",
    ]
    with ui.step(f"{title}: wrkr"):
        ui.set_current_command(label="wrkr", argv=wrkr_argv, cwd=cfg.root, env=wrkr_env)
        wrkr_res = run_with_peak_rss_sampling_streaming(
            wrkr_argv,
            cwd=cfg.root,
            env=wrkr_env,
            on_stdout_line=lambda line: ui.tail(
                _format_wrkr_json_progress_line_for_ui(line) or line
            ),
            on_stderr_line=lambda line: ui.tail(line, style="dim"),
        )

    wrkr_ok = True
    if wrkr_res.returncode != 0:
        wrkr_ok = False
        msg = f"FAIL: wrkr exited with code {wrkr_res.returncode}"
        ui.log(msg, style="red")
        failure_messages.append(msg)
        failures += 1

    wrkr_rps: Rps | None
    wrkr_json = try_parse_wrkr_json_summary(stdout=wrkr_res.stdout, stderr=wrkr_res.stderr)
    try:
        wrkr_rps = parse_wrkr_rps(
            stdout=wrkr_res.stdout,
            stderr=wrkr_res.stderr,
            test_duration_seconds=parse_duration_to_seconds(cfg.tuning.duration),
        )
    except ParseError as e:
        wrkr_rps = None
        wrkr_ok = False
        msg = f"FAIL: could not parse wrkr RPS ({e})"
        ui.log(msg, style="red")
        failure_messages.append(msg)
        failures += 1

    if wrkr_json is not None and wrkr_json.checks_failed_total > 0:
        wrkr_ok = False
        msg = f"FAIL: wrkr has failed checks (count={wrkr_json.checks_failed_total})"
        ui.log(msg, style="red")
        failure_messages.append(msg)
        failures += 1

    k6_res: RunResult | None
    k6_ok = True
    if tools.k6 is not None:
        k6_argv = [
            str(tools.k6),
            "run",
            "--vus",
            str(cfg.effective_k6_vus()),
            "--duration",
            cfg.tuning.duration,
            str(cfg.root / scripts.k6),
        ]
        with ui.step(f"{title}: k6"):
            ui.set_current_command(label="k6", argv=k6_argv, cwd=cfg.root, env=wrkr_env)
            k6_res = run_with_peak_rss_sampling_streaming(
                k6_argv,
                cwd=cfg.root,
                env=wrkr_env,
                on_stdout_line=ui.tail,
                on_stderr_line=lambda line: ui.tail(line, style="dim"),
            )

        if k6_res.returncode != 0:
            k6_ok = False
            msg = f"FAIL: k6 exited with code {k6_res.returncode}"
            ui.log(msg, style="red")
            failure_messages.append(msg)
            failures += 1
    else:
        ui.log("k6: skipped (not installed)")
        k6_res = None
        k6_ok = False

    k6_rps: Rps | None = None
    if k6_res is not None:
        try:
            k6_rps = parse_k6_grpc_rps(stdout=k6_res.stdout, stderr=k6_res.stderr)
        except ParseError as e:
            k6_rps = None
            k6_ok = False
            msg = f"FAIL: could not parse k6 RPS ({e})"
            ui.log(msg, style="red")
            failure_messages.append(msg)
            failures += 1

        k6_failed = parse_k6_grpc_req_failed_rate(stdout=k6_res.stdout, stderr=k6_res.stderr)
        if k6_failed is not None and k6_failed > 0.0:
            k6_ok = False
            msg = f"FAIL: k6 has request failures (grpc_req_failed={k6_failed:.3%})"
            ui.log(msg, style="red")
            failure_messages.append(msg)
            failures += 1

        warn_n = count_k6_request_failed_warnings(stdout=k6_res.stdout, stderr=k6_res.stderr)
        if warn_n > 0:
            k6_ok = False
            msg = f"FAIL: k6 emitted Request Failed warnings (count={warn_n})"
            ui.log(msg, style="red")
            failure_messages.append(msg)
            failures += 1

    ui.log(
        format_grpc_summary_line(
            wrkr_res=wrkr_res,
            k6_res=k6_res,
            wrkr_rps=wrkr_rps,
            k6_rps=k6_rps,
        )
    )

    summary_lines.append(f"gRPC {title}")
    summary_lines.append(
        f"  scripts: wrkr={scripts.wrkr} k6={scripts.k6} duration={cfg.tuning.duration}"
    )
    summary_lines.append(
        f"  wrkr: {'OK' if wrkr_ok else 'FAIL'} rps={wrkr_rps.value:.3f}"
        if wrkr_rps is not None
        else f"  wrkr: {'OK' if wrkr_ok else 'FAIL'} rps=-"
    )
    if wrkr_json is not None:
        summary_lines.append(
            "  wrkr json: "
            f"p50={_fmt_ms_i(wrkr_json.latency_p50_ms)}ms p90={_fmt_ms_i(wrkr_json.latency_p90_ms)}ms "
            f"p99={_fmt_ms_i(wrkr_json.latency_p99_ms)}ms max={_fmt_ms_i(wrkr_json.latency_max_ms)}ms "
            f"mean={_fmt_ms_f(wrkr_json.latency_mean_ms)}ms failed_checks={wrkr_json.checks_failed_total} "
            f"rx/s={_fmt_int(wrkr_json.bytes_received_per_sec)} tx/s={_fmt_int(wrkr_json.bytes_sent_per_sec)}"
        )
    if tools.k6 is None:
        summary_lines.append("  k6  : SKIP")
    else:
        summary_lines.append(
            f"  k6  : {'OK' if k6_ok else 'FAIL'} rps={k6_rps.value:.3f}"
            if k6_rps is not None
            else f"  k6  : {'OK' if k6_ok else 'FAIL'} rps=-"
        )

    # Gate: wrkr vs k6 (strict)
    if k6_rps is not None and wrkr_rps is not None and k6_ok and wrkr_ok:
        ratio_actual = wrkr_rps.value / k6_rps.value if k6_rps.value > 0 else float("inf")
        if is_too_slow(
            wrkr=wrkr_rps, other=k6_rps, ratio=case.ratio_ok_wrkr_over_k6, inclusive=False
        ):
            msg = (
                "FAIL: wrkr is too slow vs k6 "
                f"(ratio_ok={case.ratio_ok_wrkr_over_k6}, ratio_actual={ratio_actual:.3f})"
            )
            ui.log(msg)
            failure_messages.append(msg)
            failures += 1
        else:
            msg = f"PASS: wrkr/k6 > {case.ratio_ok_wrkr_over_k6} (ratio_actual={ratio_actual:.3f})"
            ui.log(msg)
        summary_lines.append(
            f"  gate wrkr/k6 : ratio_ok={case.ratio_ok_wrkr_over_k6} ratio_actual={ratio_actual:.3f}"
        )
    else:
        summary_lines.append("  gate wrkr/k6 : SKIP (correctness failed or missing tool)")

    return GrpcCaseOutcome(
        failures=failures,
        wrkr_rps=wrkr_rps,
        failure_messages=tuple(failure_messages),
        summary_lines=tuple(summary_lines),
    )


def is_too_slow(*, wrkr: Rps, other: Rps, ratio: float, inclusive: bool) -> bool:
    """
    Gate predicate (matches the previous tool behavior).

    inclusive=True:
      wrkr < other * ratio  => too slow
    inclusive=False:
      wrkr <= other * ratio => too slow
    """
    if inclusive:
        return (wrkr.value + 1e-15) < (other.value * ratio)
    return wrkr.value <= (other.value * ratio)


def _ensure_script_exists(root: Path, rel_path: str) -> None:
    """
    Ensure a script file exists relative to repo root.

    We validate early to avoid confusing tool failures later.
    """
    p = (root / rel_path).resolve()
    if not p.exists():
        raise CaseError(f"Missing script file: {p}")


def _print_case_header(title: str, *, ui: RunUI) -> None:
    ui.log(f"CASE: {title}")


def debug_dump_result(name: str, res: RunResult) -> None:
    """
    Optional helper for troubleshooting parsing problems locally.

    Not used by default to keep output clean.
    """
    print(f"--- {name}: returncode={res.returncode} ---", flush=True)
    print("stdout:", flush=True)
    print(res.stdout, flush=True)
    if res.stderr.strip():
        print("stderr:", flush=True)
        print(res.stderr, flush=True)


def parse_rps_or_raise(label: str, *, fn, stdout: str, stderr: str) -> Rps:
    """
    Small helper for calling parsers with a consistent error message.

    `fn` should raise ParseError on failure.
    """
    try:
        return fn(stdout=stdout, stderr=stderr)
    except ParseError as e:
        raise CaseError(f"{label}: {e}") from e
