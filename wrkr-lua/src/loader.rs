use std::path::{Path, PathBuf};

use mlua::Lua;

use crate::{Error, Result};

fn prepend_package_search_path(package: &mlua::Table, key: &str, prefix: &str) -> Result<()> {
    let prefix = prefix.trim();
    if prefix.is_empty() {
        return Ok(());
    }

    let prefix = prefix.trim_end_matches(';');
    let old: String = package.get(key)?;
    package.set(key, format!("{prefix};{old}"))?;
    Ok(())
}

fn script_dir(script_path: &Path) -> PathBuf {
    script_path
        .parent()
        .map(Path::to_path_buf)
        .unwrap_or_default()
}

fn normalize_for_lua_path(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}

pub fn chunk_name(script_path: &Path) -> String {
    let p = script_path
        .canonicalize()
        .unwrap_or_else(|_| script_path.to_path_buf());
    format!("@{}", normalize_for_lua_path(&p))
}

pub fn configure_module_path(lua: &Lua, script_path: &Path) -> Result<()> {
    let package: mlua::Table = lua.globals().get("package")?;

    if let Ok(v) = std::env::var("WRKR_LUA_PATH").or_else(|_| std::env::var("LUA_PATH")) {
        prepend_package_search_path(&package, "path", &v)?;
    }
    if let Ok(v) = std::env::var("WRKR_LUA_CPATH").or_else(|_| std::env::var("LUA_CPATH")) {
        prepend_package_search_path(&package, "cpath", &v)?;
    }

    let dir = script_dir(script_path);

    let old_path: String = package.get("path")?;

    let dir = normalize_for_lua_path(&dir);
    let prefix = format!("{dir}/?.lua;{dir}/?/init.lua;");
    package.set("path", format!("{prefix}{old_path}"))?;

    Ok(())
}

pub fn read_script_relative_text(script_path: &Path, rel: &str) -> Result<String> {
    if Path::new(rel).is_absolute() {
        return Err(Error::InvalidPath(rel.to_string()));
    }

    let Some(base_dir) = script_path.parent() else {
        return Err(Error::MissingScriptPath(rel.to_string()));
    };

    let base_dir = base_dir.canonicalize()?;
    let candidate = base_dir.join(rel).canonicalize()?;
    if !candidate.starts_with(&base_dir) {
        return Err(Error::InvalidPath(rel.to_string()));
    }

    Ok(std::fs::read_to_string(candidate)?)
}

#[cfg(test)]
mod tests {
    use super::*;
    use uuid::Uuid;

    #[test]
    fn prepend_package_search_path_puts_prefix_before_existing() {
        let lua = Lua::new();
        let package: mlua::Table = match lua.globals().get("package") {
            Ok(v) => v,
            Err(err) => panic!("lua should have package table: {err}"),
        };

        let old: String = match package.get("path") {
            Ok(v) => v,
            Err(err) => panic!("package.path exists: {err}"),
        };

        if let Err(err) = prepend_package_search_path(&package, "path", "X/?.lua") {
            panic!("prepend should work: {err}");
        }

        let new: String = match package.get("path") {
            Ok(v) => v,
            Err(err) => panic!("package.path exists after: {err}"),
        };
        assert!(new.starts_with("X/?.lua;"));
        assert!(new.ends_with(&old));
    }

    #[test]
    fn prepend_package_search_path_trims_trailing_semicolons() {
        let lua = Lua::new();
        let package: mlua::Table = match lua.globals().get("package") {
            Ok(v) => v,
            Err(err) => panic!("lua should have package table: {err}"),
        };

        if let Err(err) = prepend_package_search_path(&package, "path", "X/?.lua;") {
            panic!("prepend should work: {err}");
        }

        let new: String = match package.get("path") {
            Ok(v) => v,
            Err(err) => panic!("package.path exists after: {err}"),
        };
        assert!(new.starts_with("X/?.lua;"));
        assert!(!new.starts_with("X/?.lua;;"));
    }

    #[test]
    fn read_script_relative_text_rejects_absolute_paths() {
        let tmp = std::env::temp_dir().join(format!("wrkr-lua-test-{}", Uuid::new_v4()));
        std::fs::create_dir_all(&tmp).unwrap_or_else(|err| panic!("create temp dir: {err}"));

        let script_path = tmp.join("main.lua");
        std::fs::write(&script_path, "-- noop")
            .unwrap_or_else(|err| panic!("write main.lua: {err}"));

        let err = match read_script_relative_text(&script_path, "/etc/passwd") {
            Ok(_) => panic!("expected error"),
            Err(err) => err,
        };
        let msg = err.to_string();
        assert!(msg.contains("invalid script-relative path"), "{msg}");
    }

    #[test]
    fn read_script_relative_text_rejects_parent_traversal() {
        let parent = std::env::temp_dir().join(format!("wrkr-lua-test-{}", Uuid::new_v4()));
        let base = parent.join("base");
        std::fs::create_dir_all(&base).unwrap_or_else(|err| panic!("create temp dir: {err}"));

        // Create a real file outside `base` so `canonicalize()` succeeds.
        let secret_path = parent.join("secret.txt");
        std::fs::write(&secret_path, "secret")
            .unwrap_or_else(|err| panic!("write secret.txt: {err}"));

        let script_path = base.join("main.lua");
        std::fs::write(&script_path, "-- noop")
            .unwrap_or_else(|err| panic!("write main.lua: {err}"));

        let err = match read_script_relative_text(&script_path, "../secret.txt") {
            Ok(_) => panic!("expected error"),
            Err(err) => err,
        };
        let msg = err.to_string();
        assert!(msg.contains("invalid script-relative path"), "{msg}");
    }

    #[test]
    fn read_script_relative_text_reads_from_script_directory() {
        let tmp = std::env::temp_dir().join(format!("wrkr-lua-test-{}", Uuid::new_v4()));
        std::fs::create_dir_all(&tmp).unwrap_or_else(|err| panic!("create temp dir: {err}"));

        let script_path = tmp.join("main.lua");
        std::fs::write(&script_path, "-- noop")
            .unwrap_or_else(|err| panic!("write main.lua: {err}"));

        let payload_path = tmp.join("data.txt");
        std::fs::write(&payload_path, "hello")
            .unwrap_or_else(|err| panic!("write data.txt: {err}"));

        let got = read_script_relative_text(&script_path, "data.txt")
            .unwrap_or_else(|err| panic!("read relative: {err}"));
        assert_eq!(got, "hello");
    }
}
