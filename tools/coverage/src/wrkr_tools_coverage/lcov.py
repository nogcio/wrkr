from __future__ import annotations

from dataclasses import dataclass
from pathlib import Path


@dataclass(frozen=True)
class FileCoverage:
    path: Path
    total_lines: int
    hit_lines: int
    uncovered_lines: tuple[int, ...]

    @property
    def percent(self) -> float:
        if self.total_lines == 0:
            return 100.0
        return (self.hit_lines / self.total_lines) * 100.0


def _collapse_ranges(lines: list[int]) -> list[tuple[int, int]]:
    if not lines:
        return []

    lines = sorted(set(lines))
    ranges: list[tuple[int, int]] = []
    start = prev = lines[0]

    for line in lines[1:]:
        if line == prev + 1:
            prev = line
            continue
        ranges.append((start, prev))
        start = prev = line

    ranges.append((start, prev))
    return ranges


def summarize_lcov(
    *,
    lcov_path: Path,
    repo_root: Path,
    include_under_repo_only: bool = True,
    exclude_substrings: tuple[str, ...] = (
        "/target/",
        "/.cargo/",
        "/rustc/",
    ),
) -> list[FileCoverage]:
    """Parse an lcov file and return per-source-file line coverage.

    Notes:
    - We compute totals from DA entries (instrumented lines).
    - lcov also contains LF/LH but DA is the most direct for our purposes.
    """

    text = lcov_path.read_text(encoding="utf-8", errors="replace")

    current_sf: Path | None = None
    current_da: dict[int, int] = {}

    out: list[FileCoverage] = []

    def flush() -> None:
        nonlocal current_sf, current_da
        if current_sf is None:
            return

        src = current_sf
        current_sf = None

        # Normalize path filters.
        try:
            src_abs = src if src.is_absolute() else (repo_root / src)
            src_abs = src_abs.resolve()
        except FileNotFoundError:
            # resolve() can fail if path components don't exist; fall back.
            src_abs = (repo_root / src).absolute()

        if include_under_repo_only:
            try:
                src_abs.relative_to(repo_root.resolve())
            except ValueError:
                current_da = {}
                return

        src_str = src_abs.as_posix()
        if any(sub in src_str for sub in exclude_substrings):
            current_da = {}
            return

        total = len(current_da)
        hit = sum(1 for c in current_da.values() if c > 0)
        uncovered = tuple(sorted(k for k, v in current_da.items() if v == 0))

        out.append(
            FileCoverage(
                path=src_abs,
                total_lines=total,
                hit_lines=hit,
                uncovered_lines=uncovered,
            )
        )

        current_da = {}

    for raw in text.splitlines():
        line = raw.strip()

        if line.startswith("SF:"):
            flush()
            current_sf = Path(line[len("SF:") :])
            continue

        if line.startswith("DA:"):
            if current_sf is None:
                continue
            payload = line[len("DA:") :]
            parts = payload.split(",")
            if len(parts) < 2:
                continue
            try:
                line_no = int(parts[0])
                count = int(parts[1])
            except ValueError:
                continue

            # lcov can repeat DA lines; keep max count.
            prev = current_da.get(line_no)
            if prev is None or count > prev:
                current_da[line_no] = count
            continue

        if line == "end_of_record":
            flush()
            continue

    flush()

    # Sort: lowest coverage first, then by size.
    out.sort(key=lambda f: (f.percent, -f.total_lines, f.path.as_posix()))
    return out


def uncovered_ranges(cov: FileCoverage) -> list[tuple[int, int]]:
    return _collapse_ranges(list(cov.uncovered_lines))
