use super::types::DashboardSnapshot;

use askama::Template;

const CSS: &str = include_str!("../../assets/dashboard.css");
const JS: &str = include_str!("../../assets/dashboard.js");
const CHART: &str = include_str!("../../assets/vendor/chart.umd.min.js");

#[derive(Template)]
#[template(path = "dashboard/page.html")]
struct PageTemplate {
    css: &'static str,
    js: &'static str,
    chart: &'static str,

    title: &'static str,
    status_text: &'static str,
    mode: &'static str,
    snapshot_json: Option<String>,
}

pub(crate) fn render_live_html() -> anyhow::Result<String> {
    // Single-page app: inline assets, live connects via SSE.
    PageTemplate {
        css: CSS,
        js: JS,
        chart: CHART,
        title: "wrkr dashboard",
        status_text: "running…",
        mode: "live",
        snapshot_json: None,
    }
    .render()
    .map_err(anyhow::Error::new)
}

fn escape_json_for_html_script(json: &str) -> String {
    // Prevent breaking out of <script type="application/json"> via </script> (case-insensitive).
    json.replace('<', "\\u003c")
}

pub(crate) fn render_offline_html(snapshot: &DashboardSnapshot) -> anyhow::Result<String> {
    let raw = serde_json::to_string(snapshot)?;
    let safe = escape_json_for_html_script(&raw);

    PageTemplate {
        css: CSS,
        js: JS,
        chart: CHART,
        title: "wrkr dashboard (offline)",
        status_text: "completed",
        mode: "offline",
        snapshot_json: Some(safe),
    }
    .render()
    .map_err(anyhow::Error::new)
}
