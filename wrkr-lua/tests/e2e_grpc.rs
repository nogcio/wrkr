mod support;

use wrkr_lua::Result;
use wrkr_testserver::GrpcTestServer;

#[tokio::test]
async fn e2e_grpc_unary_echo() -> Result<()> {
    let grpc = GrpcTestServer::start().await?;

    support::run_script(
        "grpc_unary.lua",
        &[("BASE_URL", grpc.target())],
        wrkr_core::RunConfig::default(),
    )
    .await?;

    grpc.shutdown().await;
    Ok(())
}
