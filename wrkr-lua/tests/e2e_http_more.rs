mod support;

use wrkr_lua::Result;
use wrkr_testserver::TestServer;

#[tokio::test]
async fn e2e_http_more_scripts_run() -> Result<()> {
    let server = TestServer::start().await?;

    support::run_script(
        "query_params.lua",
        &[("BASE_URL", server.base_url().to_string())],
        wrkr_core::RunConfig::default(),
    )
    .await?;

    support::run_script(
        "modular.lua",
        &[("BASE_URL", server.base_url().to_string())],
        wrkr_core::RunConfig::default(),
    )
    .await?;

    support::run_script(
        "metrics_tags.lua",
        &[("BASE_URL", server.base_url().to_string())],
        wrkr_core::RunConfig::default(),
    )
    .await?;

    support::run_script(
        "wrkr_style.lua",
        &[("BASE_URL", server.base_url().to_string())],
        wrkr_core::RunConfig::default(),
    )
    .await?;

    let seen = server.stats().requests_total();
    server.shutdown().await;

    assert!(seen > 0, "expected server to see requests");
    Ok(())
}

#[tokio::test]
async fn e2e_http_ramping_scripts_run() -> Result<()> {
    let server = TestServer::start().await?;

    support::run_script(
        "ramping_vus.lua",
        &[("BASE_URL", server.base_url().to_string())],
        wrkr_core::RunConfig::default(),
    )
    .await?;

    support::run_script(
        "ramping_arrival_rate.lua",
        &[("BASE_URL", server.base_url().to_string())],
        wrkr_core::RunConfig::default(),
    )
    .await?;

    let seen = server.stats().requests_total();
    server.shutdown().await;

    assert!(seen > 0, "expected server to see requests");
    Ok(())
}

#[tokio::test]
async fn e2e_http_shared_iterations_script_runs() -> Result<()> {
    let server = TestServer::start().await?;

    support::run_script(
        "shared_iterations.lua",
        &[("BASE_URL", server.base_url().to_string())],
        wrkr_core::RunConfig::default(),
    )
    .await?;

    let seen = server.stats().requests_total();
    server.shutdown().await;

    assert!(seen > 0, "expected server to see requests");
    Ok(())
}
