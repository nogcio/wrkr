use std::collections::{BTreeMap, HashMap};
use std::net::SocketAddr;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use axum::Router;
use axum::body::Bytes;
use axum::extract::{Query, State};
use axum::http::{HeaderMap, StatusCode};
use axum::routing::{get, post};
use serde::{Deserialize, Serialize};
use tokio::net::TcpListener;
use tokio::sync::oneshot;
use tokio::time::{Duration, sleep};

pub const PATH_HELLO: &str = "/hello";
pub const PATH_PLAINTEXT: &str = "/plaintext";
pub const PATH_ECHO: &str = "/echo";
pub const PATH_SLOW: &str = "/slow";
pub const PATH_QP: &str = "/qp";
pub const PATH_ANALYTICS_AGGREGATE: &str = "/analytics/aggregate";

pub mod grpc;
pub use grpc::GrpcTestServer;

#[derive(Debug, Clone, Default)]
pub struct TestServerStats {
    requests_total: Arc<AtomicU64>,
    saw_post_header: Arc<AtomicU64>,
    saw_post_body: Arc<AtomicU64>,
    saw_json_content_type: Arc<AtomicU64>,
}

impl TestServerStats {
    fn inc_requests_total(&self) {
        self.requests_total.fetch_add(1, Ordering::Relaxed);
    }

    fn inc_saw_post_header(&self) {
        self.saw_post_header.fetch_add(1, Ordering::Relaxed);
    }

    fn inc_saw_post_body(&self) {
        self.saw_post_body.fetch_add(1, Ordering::Relaxed);
    }

    fn inc_saw_json_content_type(&self) {
        self.saw_json_content_type.fetch_add(1, Ordering::Relaxed);
    }

    pub fn requests_total(&self) -> u64 {
        self.requests_total.load(Ordering::Relaxed)
    }

    pub fn saw_post_header(&self) -> u64 {
        self.saw_post_header.load(Ordering::Relaxed)
    }

    pub fn saw_post_body(&self) -> u64 {
        self.saw_post_body.load(Ordering::Relaxed)
    }

    pub fn saw_json_content_type(&self) -> u64 {
        self.saw_json_content_type.load(Ordering::Relaxed)
    }
}

#[derive(Debug, Clone)]
pub struct TestServerUrls {
    pub base_url: String,
    pub hello: String,
    pub plaintext: String,
    pub echo: String,
    pub slow: String,
    pub qp: String,
    pub analytics_aggregate: String,
}

impl TestServerUrls {
    pub fn new(base_url: String) -> Self {
        Self {
            hello: format!("{base_url}{PATH_HELLO}"),
            plaintext: format!("{base_url}{PATH_PLAINTEXT}"),
            echo: format!("{base_url}{PATH_ECHO}"),
            slow: format!("{base_url}{PATH_SLOW}"),
            qp: format!("{base_url}{PATH_QP}"),
            analytics_aggregate: format!("{base_url}{PATH_ANALYTICS_AGGREGATE}"),
            base_url,
        }
    }
}

#[derive(Debug, Deserialize)]
struct AnalyticsAggregateRequest {
    client_id: String,
    orders: Vec<AnalyticsOrder>,
}

#[derive(Debug, Deserialize)]
struct AnalyticsOrder {
    status: i32,
    country: String,
    items: Vec<AnalyticsOrderItem>,
}

#[derive(Debug, Deserialize)]
struct AnalyticsOrderItem {
    quantity: i32,
    category: String,
    price_cents: i64,
}

#[derive(Debug, Serialize)]
struct AnalyticsAggregateResponse {
    echoed_client_id: String,
    processed_orders: i64,
    amount_by_country: BTreeMap<String, i64>,
    quantity_by_category: BTreeMap<String, i32>,
}

async fn handle_analytics_aggregate(
    State(stats): State<TestServerStats>,
    headers: HeaderMap,
    body: Bytes,
) -> (StatusCode, Bytes) {
    stats.inc_requests_total();

    // Keep the same basic header tracking behavior as /echo.
    if headers.get("x-test").and_then(|v| v.to_str().ok()) == Some("1") {
        stats.inc_saw_post_header();
    }
    if headers
        .get("content-type")
        .and_then(|v| v.to_str().ok())
        .is_some_and(|v| v.to_ascii_lowercase().starts_with("application/json"))
    {
        stats.inc_saw_json_content_type();
    }

    let req: AnalyticsAggregateRequest = match serde_json::from_slice(&body) {
        Ok(v) => v,
        Err(_) => return (StatusCode::BAD_REQUEST, Bytes::from_static(b"bad json")),
    };

    // Match the gRPC analytics behavior used by tools/perf/wfb_grpc_aggregate.lua:
    // - status == 1 => completed
    // - processed_orders counts only completed
    // - amount_by_country sums order_amount for completed orders
    // - quantity_by_category sums item.quantity for completed orders
    let mut processed_orders = 0i64;
    let mut amount_by_country: BTreeMap<String, i64> = BTreeMap::new();
    let mut quantity_by_category: BTreeMap<String, i32> = BTreeMap::new();

    for order in req.orders {
        if order.status != 1 {
            continue;
        }

        processed_orders += 1;

        let mut order_amount: i64 = 0;
        for item in order.items {
            order_amount += item.price_cents.saturating_mul(item.quantity as i64);
            *quantity_by_category.entry(item.category).or_insert(0) += item.quantity;
        }

        *amount_by_country.entry(order.country).or_insert(0) += order_amount;
    }

    let res = AnalyticsAggregateResponse {
        echoed_client_id: req.client_id,
        processed_orders,
        amount_by_country,
        quantity_by_category,
    };

    match serde_json::to_vec(&res) {
        Ok(bytes) => (StatusCode::OK, Bytes::from(bytes)),
        Err(_) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Bytes::from_static(b"encode error"),
        ),
    }
}

pub struct TestServer {
    addr: SocketAddr,
    base_url: String,
    urls: TestServerUrls,
    stats: TestServerStats,
    shutdown_tx: Option<oneshot::Sender<()>>,
    task: Option<tokio::task::JoinHandle<()>>,
}

async fn handle_hello(State(stats): State<TestServerStats>) -> &'static str {
    stats.inc_requests_total();
    "Hello World!"
}

async fn handle_plaintext(State(stats): State<TestServerStats>) -> &'static str {
    stats.inc_requests_total();
    "Hello World!"
}

async fn handle_slow(State(stats): State<TestServerStats>) -> &'static str {
    stats.inc_requests_total();
    sleep(Duration::from_millis(50)).await;
    "slow"
}

async fn handle_echo(
    State(stats): State<TestServerStats>,
    headers: HeaderMap,
    body: Bytes,
) -> (StatusCode, Bytes) {
    stats.inc_requests_total();

    if headers.get("x-test").and_then(|v| v.to_str().ok()) == Some("1") {
        stats.inc_saw_post_header();
    }
    if headers
        .get("content-type")
        .and_then(|v| v.to_str().ok())
        .is_some_and(|v| v.to_ascii_lowercase().starts_with("application/json"))
    {
        stats.inc_saw_json_content_type();
    }
    if body.as_ref() == b"ping" {
        stats.inc_saw_post_body();
    }

    (StatusCode::OK, body)
}

async fn handle_qp(
    State(stats): State<TestServerStats>,
    Query(query): Query<HashMap<String, String>>,
) -> StatusCode {
    stats.inc_requests_total();

    if query.get("foo").map(String::as_str) == Some("bar") {
        StatusCode::OK
    } else {
        StatusCode::BAD_REQUEST
    }
}

pub fn router(stats: TestServerStats) -> Router {
    Router::new()
        .route(PATH_HELLO, get(handle_hello))
        .route(PATH_PLAINTEXT, get(handle_plaintext))
        .route(PATH_SLOW, get(handle_slow))
        .route(PATH_ECHO, post(handle_echo))
        .route(PATH_ANALYTICS_AGGREGATE, post(handle_analytics_aggregate))
        .route(PATH_QP, get(handle_qp))
        .with_state(stats)
}

impl TestServer {
    pub async fn start() -> std::io::Result<Self> {
        let listener = TcpListener::bind("127.0.0.1:0").await?;
        let addr = listener.local_addr()?;

        let stats = TestServerStats::default();

        let app = router(stats.clone());

        let (shutdown_tx, shutdown_rx) = oneshot::channel::<()>();
        let task = tokio::spawn(async move {
            let serve = axum::serve(listener, app).with_graceful_shutdown(async move {
                let _ = shutdown_rx.await;
            });
            let _ = serve.await;
        });

        let base_url = format!("http://{addr}");
        let urls = TestServerUrls::new(base_url.clone());

        Ok(Self {
            addr,
            base_url,
            urls,
            stats,
            shutdown_tx: Some(shutdown_tx),
            task: Some(task),
        })
    }

    pub fn addr(&self) -> SocketAddr {
        self.addr
    }

    pub fn base_url(&self) -> &str {
        &self.base_url
    }

    pub fn urls(&self) -> &TestServerUrls {
        &self.urls
    }

    pub fn stats(&self) -> &TestServerStats {
        &self.stats
    }

    pub async fn shutdown(mut self) {
        if let Some(tx) = self.shutdown_tx.take() {
            let _ = tx.send(());
        }

        if let Some(task) = self.task.take() {
            let _ = task.await;
        }
    }
}

impl Drop for TestServer {
    fn drop(&mut self) {
        if self.shutdown_tx.is_some()
            && let Some(task) = self.task.take()
        {
            task.abort();
        }
    }
}
