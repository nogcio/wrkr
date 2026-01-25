use std::path::{Path, PathBuf};

pub(super) fn resolve_path(script_path: &Path, p: &str) -> PathBuf {
    let path = PathBuf::from(p);
    if path.is_absolute() {
        return path;
    }

    if let Some(dir) = script_path.parent() {
        return dir.join(path);
    }

    path
}
