from __future__ import annotations

from pathlib import Path

from wrkr_tools_common.repo import find_repo_root


def test_find_repo_root_from_repo_root() -> None:
    root = find_repo_root(Path.cwd())
    assert (root / "Cargo.toml").exists()
    assert (root / "wrkr").is_dir()
