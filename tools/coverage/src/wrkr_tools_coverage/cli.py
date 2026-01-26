from __future__ import annotations

import argparse
import json
import os
import subprocess
import sys
from dataclasses import asdict
from pathlib import Path

from .lcov import FileCoverage, summarize_lcov, uncovered_ranges


def _repo_root() -> Path:
    # This tool lives under tools/coverage/src/..., so repo root is 4 parents up.
    # tools/coverage/src/wrkr_tools_coverage/cli.py
    return Path(__file__).resolve().parents[4]


def _relpath(path: Path, root: Path) -> str:
    try:
        return path.resolve().relative_to(root.resolve()).as_posix()
    except ValueError:
        return path.as_posix()


def _print_text(
    *,
    items: list[FileCoverage],
    repo_root: Path,
    top: int,
    min_lines: int,
    show_ranges: int,
    relative: bool,
) -> None:
    filtered = [i for i in items if i.total_lines >= min_lines]
    filtered = filtered[:top]

    if not filtered:
        print("No files matched filters.")
        return

    print("Coverage (lowest first):")
    for cov in filtered:
        path_str = _relpath(cov.path, repo_root) if relative else cov.path.as_posix()
        pct = cov.percent
        print(f"- {path_str}: {pct:6.2f}% ({cov.hit_lines}/{cov.total_lines})")
        if show_ranges > 0 and cov.uncovered_lines:
            ranges = uncovered_ranges(cov)[:show_ranges]
            pretty = ", ".join([f"{a}" if a == b else f"{a}-{b}" for (a, b) in ranges])
            print(f"  uncovered: {pretty}")


def _print_json(*, items: list[FileCoverage], repo_root: Path, relative: bool) -> None:
    payload = []
    for cov in items:
        d = asdict(cov)
        d["path"] = _relpath(cov.path, repo_root) if relative else cov.path.as_posix()
        d["percent"] = cov.percent
        d["uncovered_ranges"] = uncovered_ranges(cov)
        payload.append(d)

    print(json.dumps(payload, indent=2, sort_keys=True))


def _cmd_gen(args: argparse.Namespace) -> int:
    repo_root = _repo_root()
    lcov_path = Path(args.lcov_path)

    cargo_args: list[str] = [
        "cargo",
        "llvm-cov",
        "--lcov",
        "--output-path",
        str(lcov_path),
    ]

    if args.workspace:
        cargo_args.append("--workspace")

    if args.package:
        for pkg in args.package:
            cargo_args.extend(["-p", pkg])

    if args.all_features:
        cargo_args.append("--all-features")

    if args.features:
        cargo_args.extend(["--features", args.features])

    if args.no_cfg_coverage:
        cargo_args.append("--no-cfg-coverage")

    print("Running:", " ".join(cargo_args))
    proc = subprocess.run(cargo_args, cwd=repo_root, check=False)
    if proc.returncode != 0:
        return proc.returncode

    if lcov_path.exists():
        print(f"Wrote {lcov_path}")
    return 0


def _cmd_report(args: argparse.Namespace) -> int:
    repo_root = _repo_root()
    lcov_path = Path(args.lcov_path)
    if not lcov_path.exists():
        print(f"lcov file not found: {lcov_path}", file=sys.stderr)
        return 2

    exclude = tuple(args.exclude_substring or [])
    default_exclude = ("/target/", "/.cargo/", "/rustc/")
    exclude = default_exclude + exclude

    items = summarize_lcov(
        lcov_path=lcov_path,
        repo_root=repo_root,
        include_under_repo_only=not args.include_external,
        exclude_substrings=exclude,
    )

    if args.format == "json":
        _print_json(items=items, repo_root=repo_root, relative=not args.absolute_paths)
        return 0

    _print_text(
        items=items,
        repo_root=repo_root,
        top=args.top,
        min_lines=args.min_lines,
        show_ranges=args.show_ranges,
        relative=not args.absolute_paths,
    )
    return 0


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(prog="wrkr-tools-coverage")
    sub = parser.add_subparsers(dest="cmd", required=True)

    p_gen = sub.add_parser("gen", help="Generate target/coverage.lcov via cargo llvm-cov")
    p_gen.add_argument(
        "--lcov-path",
        default=str(Path("target") / "coverage.lcov"),
        help="Output lcov path (default: target/coverage.lcov)",
    )
    p_gen.add_argument("--workspace", action="store_true", help="Run for --workspace")
    p_gen.add_argument("-p", "--package", action="append", help="Limit to package (repeatable)")
    p_gen.add_argument("--features", help="Cargo features")
    p_gen.add_argument("--all-features", action="store_true", help="Enable all features")
    p_gen.add_argument(
        "--no-cfg-coverage",
        action="store_true",
        help="Pass --no-cfg-coverage to cargo llvm-cov",
    )
    p_gen.set_defaults(func=_cmd_gen)

    p_rep = sub.add_parser("report", help="Report files with lowest coverage")
    p_rep.add_argument(
        "--lcov-path",
        default=str(Path("target") / "coverage.lcov"),
        help="Path to lcov file (default: target/coverage.lcov)",
    )
    p_rep.add_argument("--top", type=int, default=30, help="Show N lowest-covered files")
    p_rep.add_argument(
        "--min-lines",
        type=int,
        default=25,
        help="Ignore files with fewer total instrumented lines",
    )
    p_rep.add_argument(
        "--show-ranges",
        type=int,
        default=6,
        help="Show up to N uncovered line ranges per file (0 disables)",
    )
    p_rep.add_argument(
        "--absolute-paths",
        action="store_true",
        help="Print absolute paths instead of repo-relative",
    )
    p_rep.add_argument(
        "--include-external",
        action="store_true",
        help="Include files outside repo root (by default excluded)",
    )
    p_rep.add_argument(
        "--exclude-substring",
        action="append",
        help="Extra substrings to exclude (repeatable)",
    )
    p_rep.add_argument(
        "--format",
        choices=["text", "json"],
        default="text",
        help="Output format",
    )
    p_rep.set_defaults(func=_cmd_report)

    args = parser.parse_args(argv)

    # Help for running under uv or plain python.
    os.environ.setdefault("RUST_BACKTRACE", "1")

    return int(args.func(args))
