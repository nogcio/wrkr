"""
wrkr-tools-compare-perf (Python)

This package provides a Python + uv implementation of the perf comparison runner
that executes wrkr/wrk/k6 against a local wrkr-testserver and applies ratio gates.

Public API surface is intentionally small; prefer using the CLI entrypoint.
"""

from __future__ import annotations

__all__ = [
    "__version__",
]

# Keep in sync with pyproject.toml.
# This is intentionally a simple constant (no importlib.metadata) to avoid
# issues when running from source without installation.
__version__ = "0.1.0"
