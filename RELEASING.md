# Releasing `wrkr`

This repo publishes releases from tags `vX.Y.Z`.

Release automation is defined in:
- `.github/workflows/release.yml` (GitHub Release + binaries + Homebrew update)
- `.github/workflows/docker-publish.yml` (Docker publish on tags)

## Planning (milestones)

Use milestones named `vX.Y.Z` as the release plan.

- Add issues/PRs to a milestone only when they are intended to ship in that release.
- Keep the milestone small and credible.

## Before tagging

1) Ensure `CHANGELOG.md` is up to date

- Move relevant entries from `[Unreleased]` into a new section:
  - `## [X.Y.Z] - YYYY-MM-DD`
- Keep `[Unreleased]` as the top section for the next cycle.

2) Ensure the repo is green locally

```bash
cargo fmt --all
cargo clippy --all-targets -- -D warnings
cargo test --workspace
```

3) If dependencies changed

```bash
cargo deny check advisories
```

## Tagging and publishing

Create and push a tag:

```bash
git tag vX.Y.Z
git push origin vX.Y.Z
```

CI should:
- create a GitHub Release using `CHANGELOG.md`
- build and upload release binaries
- update the Homebrew tap formula in `nogcio/homebrew-wrkr` (`Formula/wrkr.rb`)
- publish the Docker image

## After the release

- Verify the GitHub Release assets look correct.
- Verify Homebrew formula update landed in `nogcio/homebrew-wrkr`.
- Spot-check `docker pull nogcio/wrkr:X.Y.Z` (or the version tag you published).

## Notes

- The changelog sections use `X.Y.Z` while the git tag uses `vX.Y.Z`.
- If a change is user-facing, it should appear in `CHANGELOG.md`.
