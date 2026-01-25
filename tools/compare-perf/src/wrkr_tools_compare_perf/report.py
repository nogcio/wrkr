from __future__ import annotations

from .exec import RunResult
from .parse import Rps


def _mb_from_bytes(n: int) -> float:
    return float(n) / 1024.0 / 1024.0


def _fmt_rps(rps: Rps | None) -> str:
    if rps is None:
        return "-"
    return f"{rps.value:.3f}"


def _fmt_mb_from_result(res: RunResult | None) -> str:
    if res is None:
        return "-"
    return f"{_mb_from_bytes(res.peak_rss_bytes):.2f}"


def format_http_summary_line(
    *,
    wrk_res: RunResult | None,
    wrkr_res: RunResult,
    k6_res: RunResult | None,
    wrk_rps: Rps | None,
    wrkr_rps: Rps | None,
    k6_rps: Rps | None,
) -> str:
    return (
        "summary: "
        f"rps wrk={_fmt_rps(wrk_rps)} wrkr={_fmt_rps(wrkr_rps)} k6={_fmt_rps(k6_rps)} | "
        "max_rss_mb "
        f"wrk={_fmt_mb_from_result(wrk_res)} wrkr={_mb_from_bytes(wrkr_res.peak_rss_bytes):.2f} "
        f"k6={_fmt_mb_from_result(k6_res)}"
    )


def format_grpc_summary_line(
    *,
    wrkr_res: RunResult,
    k6_res: RunResult | None,
    wrkr_rps: Rps | None,
    k6_rps: Rps | None,
) -> str:
    return (
        "summary: "
        f"rps wrkr={_fmt_rps(wrkr_rps)} k6={_fmt_rps(k6_rps)} | "
        "max_rss_mb "
        f"wrkr={_mb_from_bytes(wrkr_res.peak_rss_bytes):.2f} k6={_fmt_mb_from_result(k6_res)}"
    )


__all__ = [
    "format_grpc_summary_line",
    "format_http_summary_line",
]
