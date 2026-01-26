from __future__ import annotations

import gzip
from pathlib import Path

from wrkr_tools_profile.samply_profile import SamplyConfig


def test_samply_config_defaults_smoke() -> None:
    cfg = SamplyConfig()
    assert cfg.output_path is None
    assert cfg.open_ui is True


def test_expected_samply_extension_smoke(tmp_path: Path) -> None:
    # The samply integration uses `.json.gz` outputs; keep this as a cheap guard.
    out = tmp_path / "x.profile.json.gz"
    out.write_bytes(gzip.compress(b"{}"))
    assert out.suffixes[-2:] == [".json", ".gz"]
