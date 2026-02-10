use std::convert::Infallible;
use std::net::SocketAddr;

use axum::Router;
use axum::extract::State;
use axum::http::HeaderValue;
use axum::http::StatusCode;
use axum::response::sse::{Event, KeepAlive, Sse};
use axum::response::{Html, IntoResponse};
use axum::routing::get;
use futures_util::stream::{Stream, StreamExt as _};
use tokio::net::TcpListener;
use tokio::sync::oneshot;
use tokio_stream::wrappers::BroadcastStream;

use super::collector::DashboardCollector;
use super::templates;
use super::types::DashboardEvent;

#[derive(Debug, Clone)]
pub(crate) struct DashboardServerConfig {
    pub bind: SocketAddr,
}

pub(crate) struct DashboardServer {
    pub addr: SocketAddr,
    shutdown_tx: Option<oneshot::Sender<()>>,
    handle: tokio::task::JoinHandle<()>,
}

#[derive(Clone)]
struct AppState {
    collector: DashboardCollector,
}

impl DashboardServer {
    pub(crate) async fn start(
        collector: DashboardCollector,
        cfg: DashboardServerConfig,
    ) -> anyhow::Result<Self> {
        let listener = TcpListener::bind(cfg.bind).await?;
        let addr = listener.local_addr()?;

        let state = AppState { collector };
        let app = Router::new()
            .route("/", get(index))
            .route("/events", get(events))
            .with_state(state);

        let (shutdown_tx, shutdown_rx) = oneshot::channel::<()>();
        let handle = tokio::spawn(async move {
            let serve = axum::serve(listener, app).with_graceful_shutdown(async move {
                let _ = shutdown_rx.await;
            });
            let _ = serve.await;
        });

        Ok(Self {
            addr,
            shutdown_tx: Some(shutdown_tx),
            handle,
        })
    }

    pub(crate) async fn shutdown(mut self) {
        if let Some(tx) = self.shutdown_tx.take() {
            let _ = tx.send(());
        }
        let _ = self.handle.await;
    }
}

async fn index() -> impl IntoResponse {
    match templates::render_live_html() {
        Ok(html) => (
            [(
                axum::http::header::CONTENT_TYPE,
                HeaderValue::from_static("text/html; charset=utf-8"),
            )],
            Html(html),
        )
            .into_response(),
        Err(err) => (StatusCode::INTERNAL_SERVER_ERROR, err.to_string()).into_response(),
    }
}

async fn events(State(st): State<AppState>) -> impl IntoResponse {
    // Initial snapshot, then live ticks.
    let snapshot = st.collector.snapshot_event();
    let snapshot_json = serde_json::to_string(&snapshot).unwrap_or_else(|_| "{}".to_string());

    let first = tokio_stream::once(Ok(Event::default().event("snapshot").data(snapshot_json)));

    let rx = st.collector.subscribe();
    let collector = st.collector.clone();
    let stream = futures_util::stream::unfold(
        (BroadcastStream::new(rx), collector, false),
        |(mut rx, collector, done)| async move {
            if done {
                return None;
            }

            loop {
                let msg = rx.next().await?;
                match msg {
                    Ok(DashboardEvent::Tick { point }) => {
                        let json = serde_json::to_string(&DashboardEvent::Tick { point })
                            .unwrap_or_else(|_| "{}".to_string());
                        let evt: Result<Event, Infallible> =
                            Ok(Event::default().event("tick").data(json));
                        return Some((evt, (rx, collector, false)));
                    }
                    Ok(DashboardEvent::Done) => {
                        let json = serde_json::to_string(&DashboardEvent::Done)
                            .unwrap_or_else(|_| "{}".to_string());
                        let evt: Result<Event, Infallible> =
                            Ok(Event::default().event("done").data(json));
                        return Some((evt, (rx, collector, true)));
                    }
                    Ok(DashboardEvent::Snapshot { .. }) => {
                        // Ignore; clients already got an initial snapshot.
                        continue;
                    }
                    Err(tokio_stream::wrappers::errors::BroadcastStreamRecvError::Lagged(_)) => {
                        // Re-sync with a fresh snapshot.
                        let snap = collector.snapshot_event();
                        let json =
                            serde_json::to_string(&snap).unwrap_or_else(|_| "{}".to_string());
                        let evt: Result<Event, Infallible> =
                            Ok(Event::default().event("snapshot").data(json));
                        return Some((evt, (rx, collector, false)));
                    }
                }
            }
        },
    );

    let out = first.chain(stream);
    sse(out)
}

fn sse<S>(stream: S) -> Sse<impl Stream<Item = Result<Event, Infallible>>>
where
    S: Stream<Item = Result<Event, Infallible>> + Send + 'static,
{
    Sse::new(stream).keep_alive(KeepAlive::new().interval(std::time::Duration::from_secs(10)))
}
