use anyhow::Context as _;
use std::path::Path;

pub async fn write_file(path: &Path, contents: &str, force: bool) -> anyhow::Result<()> {
    if !force
        && tokio::fs::try_exists(path)
            .await
            .with_context(|| format!("failed to check file existence: {}", path.display()))?
    {
        anyhow::bail!(
            "refusing to overwrite existing file (use --force): {}",
            path.display()
        );
    }

    if let Some(parent) = path.parent() {
        tokio::fs::create_dir_all(parent)
            .await
            .with_context(|| format!("failed to create dir: {}", parent.display()))?;
    }

    tokio::fs::write(path, contents)
        .await
        .with_context(|| format!("failed to write file: {}", path.display()))?;

    Ok(())
}
