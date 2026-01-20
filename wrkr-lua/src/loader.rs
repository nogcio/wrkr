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

fn script_dir(script_path: Option<&Path>) -> Option<PathBuf> {
    script_path.and_then(|p| p.parent()).map(Path::to_path_buf)
}

fn normalize_for_lua_path(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}

pub fn chunk_name(script_path: Option<&Path>) -> String {
    match script_path {
        Some(p) => {
            let p = p.canonicalize().unwrap_or_else(|_| p.to_path_buf());
            format!("@{}", normalize_for_lua_path(&p))
        }
        None => "@benchmark.lua".to_string(),
    }
}

pub fn configure_module_path(lua: &Lua, script_path: Option<&Path>) -> Result<()> {
    let package: mlua::Table = lua.globals().get("package")?;

    if let Ok(v) = std::env::var("WRKR_LUA_PATH").or_else(|_| std::env::var("LUA_PATH")) {
        prepend_package_search_path(&package, "path", &v)?;
    }
    if let Ok(v) = std::env::var("WRKR_LUA_CPATH").or_else(|_| std::env::var("LUA_CPATH")) {
        prepend_package_search_path(&package, "cpath", &v)?;
    }

    let Some(dir) = script_dir(script_path) else {
        return Ok(());
    };

    let old_path: String = package.get("path")?;

    let dir = normalize_for_lua_path(&dir);
    let prefix = format!("{dir}/?.lua;{dir}/?/init.lua;");
    package.set("path", format!("{prefix}{old_path}"))?;

    Ok(())
}

pub fn read_script_relative_text(script_path: Option<&Path>, rel: &str) -> Result<String> {
    let Some(script_path) = script_path else {
        return Err(Error::MissingScriptPath(rel.to_string()));
    };

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
}
