from __future__ import annotations

from .cli import app


def main() -> None:
    """
    Console entrypoint for `wrkr-tools-compare-perf`.

    This is intentionally tiny: all CLI definitions live in `wrkr_tools_compare_perf.cli`.
    """
    app()


if __name__ == "__main__":
    main()
