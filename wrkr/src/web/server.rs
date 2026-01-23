use std::net::SocketAddr;
use std::sync::Arc;

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
use tokio::net::TcpListener;
use tokio::sync::oneshot;

use super::dashboard::Dashboard;
use super::dashboard::MAX_POINTS;

#[derive(Debug, Clone, Copy)]
pub struct WebUiConfig {
    pub bind_addr: SocketAddr,
}

#[derive(Debug)]
pub struct WebUi {
    addr: SocketAddr,
    shutdown_tx: Option<oneshot::Sender<()>>,
    task: tokio::task::JoinHandle<()>,
}

impl WebUi {
    pub async fn start(cfg: WebUiConfig, dashboard: Arc<Dashboard>) -> anyhow::Result<Self> {
        let listener = TcpListener::bind(cfg.bind_addr)
            .await
            .with_context(|| format!("failed to bind web ui: {}", cfg.bind_addr))?;
        let addr = listener
            .local_addr()
            .context("failed to resolve web ui address")?;

        let app = router(dashboard.clone());

        let (shutdown_tx, shutdown_rx) = oneshot::channel::<()>();
        let task = tokio::spawn(async move {
            let serve = axum::serve(listener, app).with_graceful_shutdown(async move {
                let _ = shutdown_rx.await;
            });
            let _ = serve.await;
        });

        Ok(Self {
            addr,
            shutdown_tx: Some(shutdown_tx),
            task,
        })
    }

    pub fn url(&self) -> String {
        format!("http://{}", self.addr)
    }

    pub async fn shutdown(mut self) {
        if let Some(tx) = self.shutdown_tx.take() {
            let _ = tx.send(());
        }

        let _ = self.task.await;
    }
}

fn router(dashboard: Arc<Dashboard>) -> Router {
    Router::new()
        .route("/", get(index))
        .route("/assets/chart.umd.min.js", get(chart_js))
        .route("/assets/dashboard.css", get(dashboard_css))
        .route("/assets/dashboard.js", get(dashboard_js))
        .route("/ws", get(ws_handler))
        .with_state(dashboard)
}

async fn index() -> impl IntoResponse {
    let tpl = IndexTemplate {
        max_points: MAX_POINTS,
    };
    match tpl.render() {
        Ok(html) => (StatusCode::OK, Html(html)).into_response(),
        Err(_) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Html("template render failed".to_string()),
        )
            .into_response(),
    }
}

async fn chart_js() -> impl IntoResponse {
    const CHART_JS: &str = include_str!("../../assets/chart.umd.min.js");
    (
        StatusCode::OK,
        [(header::CONTENT_TYPE, "text/javascript; charset=utf-8")],
        CHART_JS,
    )
}

async fn dashboard_css() -> impl IntoResponse {
    const CSS: &str = include_str!("../../assets/dashboard.css");
    (
        StatusCode::OK,
        [(header::CONTENT_TYPE, "text/css; charset=utf-8")],
        CSS,
    )
}

async fn dashboard_js() -> impl IntoResponse {
    const JS: &str = include_str!("../../assets/dashboard.js");
    (
        StatusCode::OK,
        [(header::CONTENT_TYPE, "text/javascript; charset=utf-8")],
        JS,
    )
}

async fn ws_handler(
    ws: WebSocketUpgrade,
    State(dashboard): State<Arc<Dashboard>>,
) -> impl IntoResponse {
    ws.on_upgrade(move |socket| handle_ws(socket, dashboard))
}

async fn handle_ws(mut socket: WebSocket, dashboard: Arc<Dashboard>) {
    let snapshot_msg = dashboard.snapshot_message_json();

    if socket
        .send(Message::Text(snapshot_msg.into()))
        .await
        .is_err()
    {
        return;
    }

    let mut rx = dashboard.subscribe();

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

#[derive(askama::Template)]
#[template(path = "index.html")]
struct IndexTemplate {
    max_points: usize,
}
