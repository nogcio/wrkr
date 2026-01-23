use std::{collections::HashMap, net::SocketAddr};

use tokio::net::TcpListener;
use tokio::sync::oneshot;
use tokio_stream::wrappers::TcpListenerStream;
use tonic::{Request, Response, Status};

pub mod echo {
    tonic::include_proto!("wrkr.test");
    tonic::include_proto!("_");
}

#[derive(Debug, Default)]
struct EchoSvc;

#[tonic::async_trait]
impl echo::echo_service_server::EchoService for EchoSvc {
    async fn echo(
        &self,
        request: Request<echo::EchoRequest>,
    ) -> std::result::Result<Response<echo::EchoResponse>, Status> {
        let msg = request.into_inner().message;
        Ok(Response::new(echo::EchoResponse { message: msg }))
    }
}

#[derive(Debug, Default)]
struct AnalyticsSrv;

#[tonic::async_trait]
impl echo::analytics_service_server::AnalyticsService for AnalyticsSrv {
    async fn aggregate_orders(
        &self,
        request: Request<echo::AnalyticsRequest>,
    ) -> Result<Response<echo::AggregateResult>, Status> {
        let client_id = match request.metadata().get("x-client-id") {
            Some(v) => v.to_str().unwrap_or("").to_string(),
            None => "".to_string(),
        };

        let req = request.into_inner();

        let mut processed_orders = 0;
        let mut amount_by_country: HashMap<String, i64> = HashMap::default();
        let mut quantity_by_category: HashMap<String, i32> = HashMap::default();

        for order in req.orders {
            if order.status == echo::OrderStatus::Completed as i32 {
                processed_orders += 1;

                let mut order_amount = 0;
                for item in order.items {
                    order_amount += item.price_cents * item.quantity as i64;

                    *quantity_by_category.entry(item.category).or_insert(0) += item.quantity;
                }

                *amount_by_country.entry(order.country).or_insert(0) += order_amount;
            }
        }

        let reply = echo::AggregateResult {
            processed_orders,
            amount_by_country: amount_by_country.into_iter().collect(),
            quantity_by_category: quantity_by_category.into_iter().collect(),
            echoed_client_id: client_id,
        };

        Ok(Response::new(reply))
    }
}

pub struct GrpcTestServer {
    addr: SocketAddr,
    shutdown_tx: Option<oneshot::Sender<()>>,
    task: Option<tokio::task::JoinHandle<()>>,
}

impl GrpcTestServer {
    pub async fn start() -> std::io::Result<Self> {
        let listener = TcpListener::bind("127.0.0.1:0").await?;
        let addr = listener.local_addr()?;

        let (shutdown_tx, shutdown_rx) = oneshot::channel::<()>();
        let task = tokio::spawn(async move {
            let incoming = TcpListenerStream::new(listener);

            let svc = echo::echo_service_server::EchoServiceServer::new(EchoSvc);
            let ag_svc = echo::analytics_service_server::AnalyticsServiceServer::new(AnalyticsSrv);

            let server = tonic::transport::Server::builder()
                .add_service(svc)
                .add_service(ag_svc)
                .serve_with_incoming_shutdown(incoming, async move {
                    let _ = shutdown_rx.await;
                });

            let _ = server.await;
        });

        Ok(Self {
            addr,
            shutdown_tx: Some(shutdown_tx),
            task: Some(task),
        })
    }

    pub fn addr(&self) -> SocketAddr {
        self.addr
    }

    pub fn target(&self) -> String {
        format!("{}:{}", self.addr.ip(), self.addr.port())
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

impl Drop for GrpcTestServer {
    fn drop(&mut self) {
        if self.shutdown_tx.is_some()
            && let Some(task) = self.task.take()
        {
            task.abort();
        }
    }
}
