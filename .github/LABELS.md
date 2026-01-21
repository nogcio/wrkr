# Recommended GitHub labels

This repo uses labels to keep issues/PRs searchable and to make milestone planning easy.

The intent is to keep the set **small and stable**. If a label doesn’t help filtering or planning, don’t add it.

General rule of thumb:

- Issues: set a GitHub **Issue Type** + apply **one** `p*` priority label + **one** `area:*` label (and `status:*` only when needed).
- PRs: apply `area:*` (and `breaking-change` when applicable).

## Priority

Use exactly one per issue:

- `p0` — release blocker / data loss / security / widespread breakage
- `p1` — important; should land in the next release
- `p2` — normal priority
- `p3` — nice-to-have / long tail

## Issue Type (GitHub feature)

We use GitHub’s built-in **Issue Type** field as the primary classification for “what kind of work is this?”.

Recommended issue types:

- Bug
- Feature
- Documentation
- Refactor
- Performance
- CI/Release

## Area

Use one (or a small number) based on ownership:

- `area:cli` — `wrkr/` (binary)
- `area:wrkr` — overall project / cross-cutting changes
- `area:wrkr-core` — core runner, HTTP/gRPC
- `area:wrkr-lua` — Lua engine, built-in modules, LuaLS stubs
- `area:wrkr-js` — embedded JavaScript engine (when/if present)
- `area:wrkr-py` — embedded Python engine (when/if present)
- `area:wrkr-value` — cross-language value contract
- `area:docs` — mdBook, READMEs, examples
- `area:release` — packaging, CI, Docker, Homebrew

## Status

Use when triaging/communicating blockers:

- `status:needs-triage` — new/unclassified
- `status:needs-info` — reporter needs to provide details
- `status:blocked` — waiting on external dependency / design decision

## Special

- `breaking-change` — requires release note; potentially impacts users
- `good first issue` — small, well-scoped, safe onboarding task
- `help wanted` — maintainers are happy to accept external PRs

## Usage rules

- Issues: set **Issue Type** + `p*` + `area:*` (and `status:*` only if needed).
- PRs: `area:*` (and `breaking-change` when applicable).
- Don’t encode status in milestones. Milestones represent the release plan only.
