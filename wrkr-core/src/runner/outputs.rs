use std::path::{Component, Path, PathBuf};

use super::error::{Error, Result};

fn sanitize_relative_output_path(rel: &str) -> Result<PathBuf> {
    if Path::new(rel).is_absolute() {
        return Err(Error::InvalidOutputPath(rel.to_string()));
    }

    let mut clean = PathBuf::new();
    for c in Path::new(rel).components() {
        match c {
            Component::CurDir => {}
            Component::Normal(p) => clean.push(p),
            // Forbid parent traversal and any absolute/prefix/root components.
            _ => return Err(Error::InvalidOutputPath(rel.to_string())),
        }
    }

    if clean.as_os_str().is_empty() {
        return Err(Error::InvalidOutputPath(rel.to_string()));
    }

    Ok(clean)
}

/// Writes output files produced by a script (e.g. from a `HandleSummary` hook).
///
/// All paths must be relative and must not contain parent traversal (`..`).
pub fn write_output_files(base_dir: &Path, files: &[(String, String)]) -> Result<()> {
    for (rel, content) in files {
        let rel = sanitize_relative_output_path(rel)?;
        let path = base_dir.join(rel);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(path, content)?;
    }

    Ok(())
}
