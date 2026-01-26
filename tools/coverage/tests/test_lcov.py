from __future__ import annotations

from pathlib import Path

from wrkr_tools_coverage.lcov import summarize_lcov, uncovered_ranges


def test_summarize_lcov_parses_and_sorts(tmp_path: Path) -> None:
    repo_root = tmp_path / "repo"
    repo_root.mkdir()

    # Create fake source files so resolve()/relative_to() behave.
    (repo_root / "wrkr-core" / "src").mkdir(parents=True)
    (repo_root / "wrkr-core" / "src" / "foo.rs").write_text("// foo\n")
    (repo_root / "wrkr-core" / "src" / "bar.rs").write_text("// bar\n")

    sample = Path(__file__).parent / "data" / "sample.lcov"
    lcov_text = sample.read_text(encoding="utf-8")
    lcov_text = lcov_text.replace("/repo", str(repo_root).replace("\\", "/"))

    lcov_path = tmp_path / "coverage.lcov"
    lcov_path.write_text(lcov_text, encoding="utf-8")

    items = summarize_lcov(lcov_path=lcov_path, repo_root=repo_root)
    assert len(items) == 2

    # foo has 1/4 covered, bar has 3/3 covered -> foo should come first.
    assert items[0].path.as_posix().endswith("wrkr-core/src/foo.rs")
    assert items[0].total_lines == 4
    assert items[0].hit_lines == 1

    ranges = uncovered_ranges(items[0])
    assert ranges == [(2, 3), (10, 10)]
