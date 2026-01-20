# Editor setup (LuaLS)

To get autocomplete and type hints for `require("wrkr/... ")` modules:

1. Run:

```bash
wrkr init
```

2. Open the folder in your editor.

This scaffolds:

- `.luarc.json` configured to use bundled LuaLS stubs
- `.wrkr/lua-stubs/` (LuaLS type stubs)

If you’re using VS Code, you can also add recommendations:

```bash
wrkr init --vscode
```
