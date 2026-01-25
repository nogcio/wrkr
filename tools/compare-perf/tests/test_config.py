from __future__ import annotations

import pytest

from wrkr_tools_compare_perf.config import ConfigError, env_bool, parse_duration_to_seconds


@pytest.mark.parametrize(
    ("value", "expected"),
    [
        ("200ms", 0.2),
        ("2s", 2.0),
        ("2.5s", 2.5),
        ("1m", 60.0),
        ("  5s ", 5.0),
    ],
)
def test_parse_duration_to_seconds(value: str, expected: float) -> None:
    assert parse_duration_to_seconds(value) == expected


def test_parse_duration_to_seconds_rejects_invalid() -> None:
    with pytest.raises(ConfigError):
        parse_duration_to_seconds("5")


def test_env_bool_parsing(monkeypatch: pytest.MonkeyPatch) -> None:
    monkeypatch.delenv("X", raising=False)
    assert env_bool("X", default=True) is True
    assert env_bool("X", default=False) is False

    monkeypatch.setenv("X", "true")
    assert env_bool("X", default=False) is True

    monkeypatch.setenv("X", "0")
    assert env_bool("X", default=True) is False

    monkeypatch.setenv("X", "wat")
    with pytest.raises(ConfigError):
        env_bool("X", default=True)
