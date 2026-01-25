from __future__ import annotations

import os
import re
from dataclasses import dataclass
from pathlib import Path
from typing import Final

_DURATION_RE: Final[re.Pattern[str]] = re.compile(r"^\s*(\d+(?:\.\d+)?)\s*(ms|s|m)\s*$")


class ConfigError(ValueError):
    """Raised when CLI/env configuration values are invalid."""


@dataclass(frozen=True, slots=True)
class Ratios:
    """
    Ratio gates.

    Semantics (matches the existing behavior we're replacing):
    - wrk gates are inclusive: wrkr_rps >= wrk_rps * ratio_ok
    - k6 gates are strict:     wrkr_rps >  k6_rps  * ratio_ok
    """

    ratio_ok_get_hello: float = 0.90
    ratio_ok_post_json: float = 0.90
    ratio_ok_wfb_json_aggregate: float = 0.90

    ratio_ok_wrkr_over_k6: float = 1.40
    ratio_ok_grpc_wrkr_over_k6: float = 2.00
    ratio_ok_wfb_grpc_aggregate_wrkr_over_k6: float = 1.20

    ratio_ok_grpc_wrkr_over_wrk_hello: float = 0.70


@dataclass(frozen=True, slots=True)
class ToolRequirements:
    require_wrk: bool = False
    require_k6: bool = False


@dataclass(frozen=True, slots=True)
class RunTuning:
    duration: str = "5s"

    wrkr_vus: int = 256
    k6_vus: int | None = None

    wrk_threads: int = 8
    wrk_connections: int = 256

    build: bool = True
    native: bool = True


@dataclass(frozen=True, slots=True)
class Config:
    """
    Typed config used by the runner.

    Notes:
    - `root` is the wrkr repo root (where `tools/perf/*` scripts are found and where cargo builds).
    - `duration` stays as a string for passing through to wrk/k6/wrkr, but can be validated/parsed.
    """

    root: Path
    tuning: RunTuning = RunTuning()
    ratios: Ratios = Ratios()
    requirements: ToolRequirements = ToolRequirements()

    def effective_k6_vus(self) -> int:
        return self.tuning.k6_vus if self.tuning.k6_vus is not None else self.tuning.wrkr_vus


def parse_duration_to_seconds(value: str) -> float:
    """
    Parse durations like "5s", "2.5s", "200ms", "1m" to seconds.

    This is used for validations and any rate computations that need numeric time.
    It is *not* a general-purpose parser; it intentionally supports only what this tool needs.
    """
    m = _DURATION_RE.match(value)
    if not m:
        raise ConfigError(
            f"Invalid duration {value!r}. Expected formats like '200ms', '5s', '1m' (decimals allowed)."
        )

    amount_s = float(m.group(1))
    unit = m.group(2)

    if amount_s < 0:
        raise ConfigError(f"Duration must be non-negative, got {value!r}.")

    if unit == "ms":
        return amount_s / 1000.0
    if unit == "s":
        return amount_s
    if unit == "m":
        return amount_s * 60.0

    # Should be unreachable due to regex.
    raise ConfigError(f"Unsupported duration unit in {value!r}.")


def env_path(name: str) -> Path | None:
    """
    Read a path-like env var.

    Returns None if unset or empty.
    """
    raw = os.environ.get(name)
    if not raw:
        return None
    return Path(raw)


def env_bool(name: str, *, default: bool) -> bool:
    """
    Parse boolean env vars in a predictable way.

    Truthy: 1, true, yes, y, on
    Falsy:  0, false, no, n, off
    Unset:  default
    """
    raw = os.environ.get(name)
    if raw is None:
        return default

    v = raw.strip().lower()
    if v in {"1", "true", "yes", "y", "on"}:
        return True
    if v in {"0", "false", "no", "n", "off"}:
        return False

    raise ConfigError(
        f"Invalid boolean value for {name}: {raw!r}. Expected one of "
        "'true/false', '1/0', 'yes/no', 'on/off'."
    )


def env_int(name: str) -> int | None:
    """
    Parse an optional integer env var.

    Returns None if unset/empty.
    """
    raw = os.environ.get(name)
    if raw is None or raw.strip() == "":
        return None
    try:
        return int(raw)
    except ValueError as e:
        raise ConfigError(f"Invalid integer value for {name}: {raw!r}") from e


def env_float(name: str) -> float | None:
    """
    Parse an optional float env var.

    Returns None if unset/empty.
    """
    raw = os.environ.get(name)
    if raw is None or raw.strip() == "":
        return None
    try:
        return float(raw)
    except ValueError as e:
        raise ConfigError(f"Invalid float value for {name}: {raw!r}") from e


def validate_ratios(r: Ratios) -> None:
    """
    Validate ratio gate values.

    Ratios should be positive. We allow > 1.0 (common for wrkr-over-k6 gates).
    """
    fields = (
        ("ratio_ok_get_hello", r.ratio_ok_get_hello),
        ("ratio_ok_post_json", r.ratio_ok_post_json),
        ("ratio_ok_wfb_json_aggregate", r.ratio_ok_wfb_json_aggregate),
        ("ratio_ok_wrkr_over_k6", r.ratio_ok_wrkr_over_k6),
        ("ratio_ok_grpc_wrkr_over_k6", r.ratio_ok_grpc_wrkr_over_k6),
        (
            "ratio_ok_wfb_grpc_aggregate_wrkr_over_k6",
            r.ratio_ok_wfb_grpc_aggregate_wrkr_over_k6,
        ),
        ("ratio_ok_grpc_wrkr_over_wrk_hello", r.ratio_ok_grpc_wrkr_over_wrk_hello),
    )
    for name, value in fields:
        if value <= 0:
            raise ConfigError(f"{name} must be > 0, got {value}.")


def validate_tuning(t: RunTuning) -> None:
    """
    Validate runner tuning settings.

    - durations must parse
    - VUs/threads/connections must be positive
    """
    _ = parse_duration_to_seconds(t.duration)

    if t.wrkr_vus <= 0:
        raise ConfigError(f"wrkr_vus must be > 0, got {t.wrkr_vus}.")
    if t.k6_vus is not None and t.k6_vus <= 0:
        raise ConfigError(f"k6_vus must be > 0, got {t.k6_vus}.")
    if t.wrk_threads <= 0:
        raise ConfigError(f"wrk_threads must be > 0, got {t.wrk_threads}.")
    if t.wrk_connections <= 0:
        raise ConfigError(f"wrk_connections must be > 0, got {t.wrk_connections}.")
