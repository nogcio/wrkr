use std::path::Path;

use super::collector::DashboardCollector;
use super::templates;

pub(crate) async fn write_offline_report(
    collector: &DashboardCollector,
    out_path: &Path,
) -> anyhow::Result<()> {
    let snapshot = collector.snapshot();
    let html = templates::render_offline_html(&snapshot)?;
    tokio::fs::write(out_path, html).await?;
    Ok(())
}
