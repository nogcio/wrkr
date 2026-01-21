---
name: Bug report
about: Create a report to help us reproduce and fix an issue
---

## Summary

Briefly describe the problem.

If this might be a security issue, please follow `.github/SECURITY.md` instead of filing a public issue.

## Steps to reproduce
1.
2.
3.

If possible, include a minimal Lua script and the exact command used.

```bash
# example
wrkr run ./examples/plaintext.lua
```

## Expected behavior

What should happen.

## Actual behavior

What happens instead.

Please include the relevant output/error snippet.

## Environment
- OS (e.g., macOS 14)
- Install method (Homebrew / `cargo install` / Docker / built from source)
- `wrkr --version`
- If built from source: `rustc -V` and `cargo -V`
- Commands to reproduce

## Area (optional)
- [ ] CLI (`wrkr/`)
- [ ] Core runner (`wrkr-core/`)
- [ ] Lua engine / modules (`wrkr-lua/`)
- [ ] Docs / examples
- [ ] CI / release

## Logs / additional information
Attach logs, stack traces or a minimal reproduction.
