from __future__ import annotations

from pathlib import Path

import pytest

from wrkr_tools_compare_perf.parse import (
    ParseError,
    detect_wrk_errors,
    parse_k6_http_rps,
    parse_wrk_rps,
    parse_wrkr_rps,
    try_parse_wrkr_json_summary,
)


def _read_fixture(name: str) -> str:
    fixtures_dir = Path(__file__).parent / "fixtures"
    return (fixtures_dir / name).read_text(encoding="utf-8")


def test_parse_wrk_rps_happy_path() -> None:
    out = _read_fixture("wrk_ok_stdout.txt")
    assert parse_wrk_rps(out).value == pytest.approx(12345.67)


def test_detect_wrk_errors_non_2xx_and_socket_errors() -> None:
    out = _read_fixture("wrk_errors_stdout.txt")
    errs = detect_wrk_errors(out)
    assert "wrk non-2xx/3xx responses: 2" in errs
    assert "wrk socket read: 12" in errs


def test_parse_k6_http_rps_from_http_reqs() -> None:
    out = _read_fixture("k6_http_stdout.txt")
    rps = parse_k6_http_rps(stdout=out, stderr="")
    assert rps.value == pytest.approx(217130.6)


def test_parse_k6_http_rps_raises_on_unparseable() -> None:
    with pytest.raises(ParseError):
        parse_k6_http_rps(stdout="nope", stderr="")


def test_parse_wrkr_rps_from_json_ndjson() -> None:
    out = _read_fixture("wrkr_json_stdout.ndjson")
    rps = parse_wrkr_rps(stdout=out, stderr="")
    assert rps.value == pytest.approx(120.0)


def test_try_parse_wrkr_json_summary() -> None:
    out = _read_fixture("wrkr_json_stdout.ndjson")
    s = try_parse_wrkr_json_summary(stdout=out, stderr="")
    assert s is not None
    assert s.elapsed_secs == 5
    assert s.total_requests == 600
    assert s.rps == pytest.approx(120.0)
    assert s.checks_failed_total == 0
    assert s.latency_p99_ms == 9
