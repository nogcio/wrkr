use std::net::SocketAddr;

use tokio::net::TcpListener;
use tokio::sync::oneshot;
use tokio_stream::wrappers::TcpListenerStream;
use tonic::{Request, Response, Status};

pub mod echo {
    tonic::include_proto!("wrkr.test");
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

            let server = tonic::transport::Server::builder()
                .add_service(svc)
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
