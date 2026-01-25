from __future__ import annotations

import sys

from .cli import app


def main() -> None:
    app()


if __name__ == "__main__":
    sys.exit(main())
