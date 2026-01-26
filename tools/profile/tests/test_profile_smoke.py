from __future__ import annotations

import pytest

from wrkr_tools_profile.profile import ProfileConfig, run_profile


def test_profile_config_dataclass_smoke() -> None:
    cfg = ProfileConfig(
        sample_duration_seconds=1,
        load_duration="1s",
        vus=1,
        script="tools/perf/wrkr_grpc_plaintext.lua",
        pre_sample_sleep_seconds=0,
    )
    assert cfg.vus == 1


@pytest.mark.skipif(True, reason="integration: requires building + running binaries")
def test_run_profile_integration() -> None:
    # Intentionally disabled by default; kept as a template for local runs.
    run_profile(
        ProfileConfig(
            sample_duration_seconds=1,
            load_duration="2s",
            vus=1,
            script="tools/perf/wrkr_grpc_plaintext.lua",
        )
    )
