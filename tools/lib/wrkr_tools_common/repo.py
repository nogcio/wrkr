from __future__ import annotations

from pathlib import Path


class RepoError(RuntimeError):
    """Raised when the wrkr repository root cannot be located."""


def find_repo_root(start: Path | None = None) -> Path:
    """Find the wrkr repository root.

    The root is identified as a directory that contains a top-level `Cargo.toml`
    and a `wrkr/` crate directory.
    """
    cwd = start or Path.cwd()
    for parent in [cwd, *cwd.parents]:
        if (parent / "Cargo.toml").exists() and (parent / "wrkr").is_dir():
            return parent
    raise RepoError("Could not find wrkr repository root (no Cargo.toml + wrkr/ found).")
