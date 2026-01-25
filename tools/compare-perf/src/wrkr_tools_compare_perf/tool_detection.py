from __future__ import annotations

import os
import shutil
from dataclasses import dataclass
from pathlib import Path

from .config import ToolRequirements


class ToolDetectionError(RuntimeError):
    """Raised when required tools/binaries cannot be found."""


@dataclass(frozen=True, slots=True)
class ToolPaths:
    """Resolved tool paths.

    - `wrkr` and `wrkr_testserver` are expected to be built from the repo and live under
      `{root}/target/release/`.
    - `wrk` and `k6` are optional external tools detected on PATH unless required.
    """

    wrk: Path | None
    k6: Path | None
    wrkr: Path
    wrkr_testserver: Path


def detect_tools(root: Path, requirements: ToolRequirements) -> ToolPaths:
    """Detect required binaries and optional external tools.

    Parameters
    ----------
    root:
        wrkr repository root directory.
    requirements:
        Whether `wrk` and/or `k6` are required.

    Returns
    -------
    ToolPaths

    Raises
    ------
    ToolDetectionError
        If required binaries/tools are missing.
    """
    root = root.resolve()

    wrkr = root / "target" / "release" / _exe_name("wrkr")
    if not wrkr.exists():
        raise ToolDetectionError(
            f"Missing binary: {wrkr} (build first or pass --build so it can be built automatically)"
        )

    wrkr_testserver = root / "target" / "release" / _exe_name("wrkr-testserver")
    if not wrkr_testserver.exists():
        raise ToolDetectionError(
            f"Missing binary: {wrkr_testserver} (build first or pass --build so it can be built automatically)"
        )

    wrk = _which("wrk")
    if requirements.require_wrk and wrk is None:
        raise ToolDetectionError("Missing required command: wrk (not found on PATH)")

    k6 = _which("k6")
    if requirements.require_k6 and k6 is None:
        raise ToolDetectionError("Missing required command: k6 (not found on PATH)")

    return ToolPaths(
        wrk=wrk,
        k6=k6,
        wrkr=wrkr,
        wrkr_testserver=wrkr_testserver,
    )


def _exe_name(base: str) -> str:
    """Return platform-specific executable name."""
    if os.name == "nt" and not base.lower().endswith(".exe"):
        return f"{base}.exe"
    return base


def _which(cmd: str) -> Path | None:
    """Find an executable on PATH."""
    found = shutil.which(cmd)
    if not found:
        return None
    return Path(found)
