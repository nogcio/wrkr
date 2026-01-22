use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::{Arc, RwLock};

use anyhow::Context as _;
use askama::Template;
use axum::Router;
use axum::extract::State;
use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::http::StatusCode;
use axum::http::header;
use axum::response::{Html, IntoResponse};
use axum::routing::get;
use futures_util::StreamExt as _;
use serde::Serialize;
use tokio::net::TcpListener;
use tokio::sync::{broadcast, oneshot};

const MAX_POINTS: usize = 300;

#[derive(Debug, Clone, Copy)]
pub struct WebUiConfig {
    pub bind_addr: SocketAddr,
}

#[derive(Debug)]
pub struct WebUi {
    addr: SocketAddr,
    state: Arc<AppState>,
    shutdown_tx: Option<oneshot::Sender<()>>,
    task: tokio::task::JoinHandle<()>,
}

impl WebUi {
    pub async fn start(cfg: WebUiConfig) -> anyhow::Result<Self> {
        let listener = TcpListener::bind(cfg.bind_addr)
            .await
            .with_context(|| format!("failed to bind web ui: {}", cfg.bind_addr))?;
        let addr = listener
            .local_addr()
            .context("failed to resolve web ui address")?;

        let (tx, _rx) = broadcast::channel::<String>(1024);
        let state = Arc::new(AppState {
            scenarios: RwLock::new(HashMap::new()),
            tx,
        });

        let app = router(state.clone());

        let (shutdown_tx, shutdown_rx) = oneshot::channel::<()>();
        let task = tokio::spawn(async move {
            let serve = axum::serve(listener, app).with_graceful_shutdown(async move {
                let _ = shutdown_rx.await;
            });
            let _ = serve.await;
        });

        Ok(Self {
            addr,
            state,
            shutdown_tx: Some(shutdown_tx),
            task,
        })
    }

    pub fn url(&self) -> String {
        format!("http://{}", self.addr)
    }

    pub fn progress_fn(&self) -> wrkr_core::runner::ProgressFn {
        let state = self.state.clone();
        Arc::new(move |u| state.on_progress(u))
    }

    pub fn notify_done(&self) {
        let msg = serde_json::json!({ "type": "done" }).to_string();
        let _ = self.state.tx.send(msg);
    }

    pub async fn shutdown(mut self) {
        if let Some(tx) = self.shutdown_tx.take() {
            let _ = tx.send(());
        }

        let _ = self.task.await;
    }
}

#[derive(Debug)]
struct AppState {
    scenarios: RwLock<HashMap<String, ScenarioState>>,
    tx: broadcast::Sender<String>,
}

impl AppState {
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
}

fn router(state: Arc<AppState>) -> Router {
    Router::new()
        .route("/", get(index))
        .route("/assets/chart.umd.min.js", get(chart_js))
        .route("/ws", get(ws_handler))
        .with_state(state)
}

async fn index() -> Html<String> {
    let tpl = IndexTemplate {
        max_points: MAX_POINTS,
    };
    let html = match tpl.render() {
        Ok(v) => v,
        Err(_) => "template render failed".to_string(),
    };
    Html(html)
}

async fn chart_js() -> impl IntoResponse {
    const CHART_JS: &str = include_str!("../../assets/chart.umd.min.js");
    (
        StatusCode::OK,
        [(header::CONTENT_TYPE, "text/javascript; charset=utf-8")],
        CHART_JS,
    )
}

async fn ws_handler(ws: WebSocketUpgrade, State(state): State<Arc<AppState>>) -> impl IntoResponse {
    ws.on_upgrade(move |socket| handle_ws(socket, state))
}

async fn handle_ws(mut socket: WebSocket, state: Arc<AppState>) {
    let snapshot_msg = {
        let guard = state
            .scenarios
            .read()
            .unwrap_or_else(|poisoned| poisoned.into_inner());

        let scenarios: HashMap<String, ScenarioSnapshot> = guard
            .iter()
            .map(|(k, v)| (k.clone(), v.snapshot()))
            .collect();

        serde_json::to_string(&WsMessage::Snapshot { scenarios })
            .unwrap_or_else(|_| r#"{"type":"snapshot","scenarios":{}}"#.to_string())
    };

    if socket
        .send(Message::Text(snapshot_msg.into()))
        .await
        .is_err()
    {
        return;
    }

    let mut rx = state.tx.subscribe();

    loop {
        tokio::select! {
            recv = rx.recv() => {
                let Ok(text) = recv else {
                    break;
                };
                if socket
                    .send(Message::Text(text.into()))
                    .await
                    .is_err()
                {
                    break;
                }
            }
            incoming = socket.next() => {
                let Some(Ok(msg)) = incoming else {
                    break;
                };
                match msg {
                    Message::Close(_) => break,
                    Message::Ping(payload) => {
                        let _ = socket.send(Message::Pong(payload)).await;
                    }
                    _ => {}
                }
            }
        }
    }
}

#[derive(Debug, Clone, Serialize)]
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
            if self.rps.len() > MAX_POINTS {
                let over = self.rps.len() - MAX_POINTS;
                self.rps.drain(0..over);
            }
        }

        let push_opt = |series: &mut Vec<Point>, y_opt: Option<f64>| {
            if let Some(y) = y_opt
                && y.is_finite()
            {
                series.push(Point { x, y });
                if series.len() > MAX_POINTS {
                    let over = series.len() - MAX_POINTS;
                    series.drain(0..over);
                }
            }
        };

        push_opt(&mut self.lat_p50, row.latency_p50_ms_now);
        push_opt(&mut self.lat_p90, row.latency_p90_ms_now);
        push_opt(&mut self.lat_p95, row.latency_p95_ms_now);
        push_opt(&mut self.lat_p99, row.latency_p99_ms_now);

        let err_pct = row.error_rate_now * 100.0;
        if err_pct.is_finite() {
            self.error_rate.push(Point { x, y: err_pct });
            if self.error_rate.len() > MAX_POINTS {
                let over = self.error_rate.len() - MAX_POINTS;
                self.error_rate.drain(0..over);
            }
        }

        self.vus.push(Point {
            x,
            y: row.vus_current as f64,
        });
        if self.vus.len() > MAX_POINTS {
            let over = self.vus.len() - MAX_POINTS;
            self.vus.drain(0..over);
        }

        let iters_y = row.iterations_per_sec_now;
        if iters_y.is_finite() {
            self.iters.push(Point { x, y: iters_y });
            if self.iters.len() > MAX_POINTS {
                let over = self.iters.len() - MAX_POINTS;
                self.iters.drain(0..over);
            }
        }

        self.rx.push(Point {
            x,
            y: row.bytes_received_per_sec_now as f64,
        });
        if self.rx.len() > MAX_POINTS {
            let over = self.rx.len() - MAX_POINTS;
            self.rx.drain(0..over);
        }

        self.tx.push(Point {
            x,
            y: row.bytes_sent_per_sec_now as f64,
        });
        if self.tx.len() > MAX_POINTS {
            let over = self.tx.len() - MAX_POINTS;
            self.tx.drain(0..over);
        }
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

#[derive(Debug, Clone, Serialize)]
struct ScenarioSnapshot {
    latest: Row,
    series: ScenarioSeries,
}

#[derive(Debug, Clone, Serialize)]
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
#[template(path = "index.html")]
struct IndexTemplate {
    max_points: usize,
}
