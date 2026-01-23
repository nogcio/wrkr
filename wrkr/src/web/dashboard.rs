use std::collections::HashMap;
use std::sync::RwLock;

use anyhow::Context as _;
use askama::Template;
use serde::Serialize;
use tokio::sync::broadcast;

pub const MAX_POINTS: usize = 300;

#[derive(Debug)]
pub struct Dashboard {
    scenarios: RwLock<HashMap<String, ScenarioState>>,
    tx: broadcast::Sender<String>,
}

impl Dashboard {
    pub fn new() -> Self {
        let (tx, _rx) = broadcast::channel::<String>(1024);
        Self {
            scenarios: RwLock::new(HashMap::new()),
            tx,
        }
    }

    pub fn progress_fn(self: &std::sync::Arc<Self>) -> wrkr_core::runner::ProgressFn {
        let dashboard = self.clone();
        std::sync::Arc::new(move |u| dashboard.on_progress(u))
    }

    pub fn subscribe(&self) -> broadcast::Receiver<String> {
        self.tx.subscribe()
    }

    pub fn notify_done(&self) {
        let msg = match serde_json::to_string(&WsMessage::Done) {
            Ok(v) => v,
            Err(_) => r#"{"type":"done"}"#.to_string(),
        };
        let _ = self.tx.send(msg);
    }

    pub fn snapshot_message_json(&self) -> String {
        let scenarios = self.snapshot_scenarios();
        serde_json::to_string(&WsMessage::Snapshot { scenarios })
            .unwrap_or_else(|_| r#"{"type":"snapshot","scenarios":{}}"#.to_string())
    }

    pub fn render_offline_html(&self) -> anyhow::Result<String> {
        let snapshot_json = self
            .snapshot_message_json()
            .replace("</script", "<\\/script");
        let tpl = OfflineTemplate {
            max_points: MAX_POINTS,
            snapshot_json: &snapshot_json,
            chart_js: include_str!("../../assets/chart.umd.min.js"),
            dashboard_css: include_str!("../../assets/dashboard.css"),
            dashboard_js: include_str!("../../assets/dashboard.js"),
        };
        tpl.render().context("render offline dashboard html")
    }

    fn on_progress(&self, u: wrkr_core::runner::ProgressUpdate) {
        let row = Row::from_progress(&u);
        let scenario = u.scenario.clone();

        {
            let mut guard = self
                .scenarios
                .write()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            let st = guard
                .entry(scenario.clone())
                .or_insert_with(|| ScenarioState::new(row.clone()));
            st.latest = row.clone();
            st.push_point(&row);
        }

        let msg = match serde_json::to_string(&WsMessage::Update {
            scenario,
            tick: u.tick,
            data: Box::new(row),
        }) {
            Ok(v) => v,
            Err(_) => return,
        };
        let _ = self.tx.send(msg);
    }

    fn snapshot_scenarios(&self) -> HashMap<String, ScenarioSnapshot> {
        let guard = self
            .scenarios
            .read()
            .unwrap_or_else(|poisoned| poisoned.into_inner());

        guard
            .iter()
            .map(|(k, v)| (k.clone(), v.snapshot()))
            .collect()
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct Row {
    exec: String,
    elapsed_secs: u64,

    vus_current: u64,
    vus_max: Option<u64>,
    dropped_iterations_total: Option<u64>,

    rps_now: f64,
    failed_rps_now: f64,
    error_rate_now: f64,
    bytes_received_per_sec_now: u64,
    bytes_sent_per_sec_now: u64,

    requests_total: u64,
    failed_requests_total: u64,
    checks_failed_total: u64,

    latency_p50_ms_now: Option<f64>,
    latency_p90_ms_now: Option<f64>,
    latency_p95_ms_now: Option<f64>,
    latency_p99_ms_now: Option<f64>,

    iterations_total: u64,
    iterations_per_sec_now: f64,

    errors_now: HashMap<String, u64>,
}

impl Row {
    fn from_progress(u: &wrkr_core::runner::ProgressUpdate) -> Self {
        let (vus_current, vus_max, dropped_iterations_total) = match &u.progress {
            wrkr_core::runner::ScenarioProgress::ConstantVus { vus, .. } => {
                (*vus, Some(*vus), None)
            }
            wrkr_core::runner::ScenarioProgress::RampingVus { stage, .. } => {
                let current = stage.as_ref().map(|s| s.current_target).unwrap_or(0);
                (current, None, None)
            }
            wrkr_core::runner::ScenarioProgress::RampingArrivalRate {
                active_vus,
                max_vus,
                dropped_iterations_total,
                ..
            } => (*active_vus, Some(*max_vus), Some(*dropped_iterations_total)),
        };

        Self {
            exec: u.exec.clone(),
            elapsed_secs: u.elapsed.as_secs(),
            vus_current,
            vus_max,
            dropped_iterations_total,
            rps_now: u.metrics.rps_now,
            failed_rps_now: u.metrics.failed_rps_now,
            error_rate_now: u.metrics.error_rate_now,
            bytes_received_per_sec_now: u.metrics.bytes_received_per_sec_now,
            bytes_sent_per_sec_now: u.metrics.bytes_sent_per_sec_now,
            requests_total: u.metrics.requests_total,
            failed_requests_total: u.metrics.failed_requests_total,
            checks_failed_total: u.metrics.checks_failed_total,
            latency_p50_ms_now: u.metrics.latency_p50_ms_now,
            latency_p90_ms_now: u.metrics.latency_p90_ms_now,
            latency_p95_ms_now: u.metrics.latency_p95_ms_now,
            latency_p99_ms_now: u.metrics.latency_p99_ms_now,
            iterations_total: u.metrics.iterations_total,
            iterations_per_sec_now: u.metrics.iterations_per_sec_now,
            errors_now: u.metrics.errors_now.clone(),
        }
    }
}

#[derive(Debug, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum WsMessage {
    Snapshot {
        scenarios: HashMap<String, ScenarioSnapshot>,
    },
    Update {
        scenario: String,
        tick: u64,
        data: Box<Row>,
    },
    Done,
}

#[derive(Debug, Clone)]
struct ScenarioState {
    latest: Row,
    rps: Vec<Point>,
    lat_p50: Vec<Point>,
    lat_p90: Vec<Point>,
    lat_p95: Vec<Point>,
    lat_p99: Vec<Point>,
    error_rate: Vec<Point>,
    vus: Vec<Point>,
    iters: Vec<Point>,
    rx: Vec<Point>,
    tx: Vec<Point>,
}

impl ScenarioState {
    fn new(latest: Row) -> Self {
        Self {
            latest,
            rps: Vec::new(),
            lat_p50: Vec::new(),
            lat_p90: Vec::new(),
            lat_p95: Vec::new(),
            lat_p99: Vec::new(),
            error_rate: Vec::new(),
            vus: Vec::new(),
            iters: Vec::new(),
            rx: Vec::new(),
            tx: Vec::new(),
        }
    }

    fn push_point(&mut self, row: &Row) {
        let x = row.elapsed_secs;

        let rps_y = row.rps_now;
        if rps_y.is_finite() {
            self.rps.push(Point { x, y: rps_y });
            trim_to_max(&mut self.rps);
        }

        fn push_opt(series: &mut Vec<Point>, x: u64, y_opt: Option<f64>) {
            if let Some(y) = y_opt
                && y.is_finite()
            {
                series.push(Point { x, y });
                trim_to_max(series);
            }
        }

        push_opt(&mut self.lat_p50, x, row.latency_p50_ms_now);
        push_opt(&mut self.lat_p90, x, row.latency_p90_ms_now);
        push_opt(&mut self.lat_p95, x, row.latency_p95_ms_now);
        push_opt(&mut self.lat_p99, x, row.latency_p99_ms_now);

        let err_pct = row.error_rate_now * 100.0;
        if err_pct.is_finite() {
            self.error_rate.push(Point { x, y: err_pct });
            trim_to_max(&mut self.error_rate);
        }

        self.vus.push(Point {
            x,
            y: row.vus_current as f64,
        });
        trim_to_max(&mut self.vus);

        let iters_y = row.iterations_per_sec_now;
        if iters_y.is_finite() {
            self.iters.push(Point { x, y: iters_y });
            trim_to_max(&mut self.iters);
        }

        self.rx.push(Point {
            x,
            y: row.bytes_received_per_sec_now as f64,
        });
        trim_to_max(&mut self.rx);

        self.tx.push(Point {
            x,
            y: row.bytes_sent_per_sec_now as f64,
        });
        trim_to_max(&mut self.tx);
    }

    fn snapshot(&self) -> ScenarioSnapshot {
        ScenarioSnapshot {
            latest: self.latest.clone(),
            series: ScenarioSeries {
                rps: self.rps.clone(),
                lat_p50: self.lat_p50.clone(),
                lat_p90: self.lat_p90.clone(),
                lat_p95: self.lat_p95.clone(),
                lat_p99: self.lat_p99.clone(),
                error_rate: self.error_rate.clone(),
                vus: self.vus.clone(),
                iters: self.iters.clone(),
                rx: self.rx.clone(),
                tx: self.tx.clone(),
            },
        }
    }
}

fn trim_to_max(series: &mut Vec<Point>) {
    if series.len() <= MAX_POINTS {
        return;
    }
    let over = series.len() - MAX_POINTS;
    series.drain(0..over);
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct ScenarioSnapshot {
    latest: Row,
    series: ScenarioSeries,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct ScenarioSeries {
    rps: Vec<Point>,
    lat_p50: Vec<Point>,
    lat_p90: Vec<Point>,
    lat_p95: Vec<Point>,
    lat_p99: Vec<Point>,
    error_rate: Vec<Point>,
    vus: Vec<Point>,
    iters: Vec<Point>,
    rx: Vec<Point>,
    tx: Vec<Point>,
}

#[derive(Debug, Clone, Serialize)]
struct Point {
    x: u64,
    y: f64,
}

#[derive(askama::Template)]
#[template(path = "offline.html")]
struct OfflineTemplate<'a> {
    max_points: usize,
    snapshot_json: &'a str,
    chart_js: &'a str,
    dashboard_css: &'a str,
    dashboard_js: &'a str,
}
