# Maintainer guide (issues, milestones, releases)

This document is for maintainers and project managers.

See also:
- `.github/LABELS.md` (label taxonomy)
- `RELEASING.md` (step-by-step release process)

## Triage (new issues)

Goal: keep the issue tracker actionable and easy to navigate.

Suggested triage steps:

1) **Classify**
- Set the GitHub **Issue type** (Bug/Feature/Documentation/etc).
- Apply an `area:*` label (`area:wrkr-core`, `area:wrkr-lua`, `area:cli`, `area:docs`, `area:release`).

2) **Prioritize**
- Apply `p0`..`p3` (or your equivalent). Keep the meaning consistent:
  - `p0`: blocks users / security / release blocker
  - `p1`: important, should land in the next release
  - `p2`: normal priority
  - `p3`: nice-to-have / long tail

3) **Make it actionable**
- If reproduction/details are missing, add `status:needs-info` and ask for specifics.
- If it’s a question (“how do I…?”), move it to Discussions and close the issue.
- If it’s a duplicate, link the canonical issue and close.

4) **Mark contribution-friendly work**
- Use `good first issue` for small, well-scoped tasks.
- Use `help wanted` when you’re happy to accept an external PR.

## Milestones (release planning)

Use milestones as the release plan.

- Create milestones named like `vX.Y.Z`.
- Put issues and PRs into a milestone only when you expect them to ship in that release.
- Prefer a small, credible milestone over a huge backlog dump.

Release readiness checklist per milestone:
- All items are either merged, explicitly deferred, or clearly blocked.
- Breaking changes are called out (issue label like `breaking-change`).
- Docs-impacting changes are tracked.

## PR review policy (1 → 2 maintainers)

While there is 1 maintainer:
- Keep PRs small and require the CI checks to pass.
- For risky changes, ask contributors to include benchmarks or extra tests.

Once there are 2 maintainers:
- Prefer **at least 1 approving review** before merge.
- Consider adding `CODEOWNERS` to route reviews by area.

## Release process (high level)

1) Ensure the milestone is ready.
2) Ensure the repo is green:
- `cargo fmt --all`
- `cargo clippy --all-targets -- -D warnings`
- `cargo test --workspace`

3) Dependency safety (only if dependencies changed):
- `cargo deny check advisories`

4) Update release notes (CHANGELOG / GitHub Release notes).
5) Tag and publish:
- Create a tag `vX.Y.Z` and publish the GitHub Release.
- CI workflows should build artifacts / Docker / Homebrew as configured.

## Recommended repo settings

- Protect `main`.
- Require PRs (no direct pushes).
- Require status checks (fmt/clippy/test + any security checks you run).
- Optional: require at least 1 review once you have 2 maintainers.
