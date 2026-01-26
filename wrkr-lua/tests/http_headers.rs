mod support;

use wrkr_lua::Result;
use wrkr_testserver::TestServer;

#[tokio::test]
async fn http_response_includes_headers_table() -> Result<()> {
    let server = TestServer::start().await?;

    support::run_script(
        "http_headers.lua",
        &[("BASE_URL", server.base_url().to_string())],
        wrkr_core::RunConfig::default(),
    )
    .await?;

    server.shutdown().await;
    Ok(())
}
