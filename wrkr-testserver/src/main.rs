use std::net::SocketAddr;

use tokio::net::TcpListener;

#[tokio::main(flavor = "current_thread")]
async fn main() -> anyhow::Result<()> {
    let mut bind_addr: SocketAddr = "127.0.0.1:0".parse()?;

    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--bind" => {
                let addr = args.next().ok_or_else(|| {
                    anyhow::anyhow!("--bind requires an address, e.g. 127.0.0.1:0")
                })?;
                bind_addr = addr.parse()?;
            }
            "-h" | "--help" => {
                eprintln!(
                    "wrkr-testserver\n\nUSAGE:\n  wrkr-testserver [--bind 127.0.0.1:0]\n\nOUTPUT:\n  Prints HTTP_URL=<url> and GRPC_URL=<host:port> to stdout once ready."
                );
                return Ok(());
            }
            other => {
                return Err(anyhow::anyhow!("unknown argument: {other}"));
            }
        }
    }

    let listener = TcpListener::bind(bind_addr).await?;
    let addr = listener.local_addr()?;

    let grpc = wrkr_testserver::GrpcTestServer::start().await?;

    let stats = wrkr_testserver::TestServerStats::default();
    let app = wrkr_testserver::router(stats);

    println!("HTTP_URL=http://{addr}");
    println!("GRPC_URL={}", grpc.target());

    let serve = axum::serve(listener, app).with_graceful_shutdown(async move {
        let _ = tokio::signal::ctrl_c().await;
    });

    serve.await?;

    grpc.shutdown().await;
    Ok(())
}
