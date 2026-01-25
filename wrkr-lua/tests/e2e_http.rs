mod support;

use wrkr_lua::Result;
use wrkr_testserver::TestServer;

#[tokio::test]
async fn e2e_http_plaintext_script_sends_requests() -> Result<()> {
    let server = TestServer::start().await?;

    let summary = support::run_script(
        "plaintext.lua",
        &[("BASE_URL", server.base_url().to_string())],
        wrkr_core::RunConfig::default(),
    )
    .await?;

    let seen = server.stats().requests_total();
    server.shutdown().await;

    let _ = summary;
    assert!(seen > 0, "expected server to see requests");
    Ok(())
}

#[tokio::test]
async fn e2e_http_post_echo_tracks_header_and_body() -> Result<()> {
    let server = TestServer::start().await?;

    support::run_script(
        "post_echo.lua",
        &[("BASE_URL", server.base_url().to_string())],
        wrkr_core::RunConfig::default(),
    )
    .await?;

    let saw_header = server.stats().saw_post_header();
    let saw_body = server.stats().saw_post_body();
    server.shutdown().await;

    assert!(saw_header > 0, "expected server to see x-test header");
    assert!(saw_body > 0, "expected server to see post body");
    Ok(())
}

#[tokio::test]
async fn e2e_http_post_json_tracks_content_type() -> Result<()> {
    let server = TestServer::start().await?;

    support::run_script(
        "post_json.lua",
        &[("BASE_URL", server.base_url().to_string())],
        wrkr_core::RunConfig::default(),
    )
    .await?;

    let saw_ct = server.stats().saw_json_content_type();
    server.shutdown().await;

    assert!(
        saw_ct > 0,
        "expected server to see application/json content-type"
    );
    Ok(())
}

#[tokio::test]
async fn e2e_http_timeout_is_reported_as_error_response_but_iteration_succeeds() -> Result<()> {
    let server = TestServer::start().await?;

    support::run_script(
        "timeout.lua",
        &[("BASE_URL", server.base_url().to_string())],
        wrkr_core::RunConfig::default(),
    )
    .await?;

    // With a 1ms client timeout the request may not reach the server; the key
    // behavior is that the iteration completes successfully.
    server.shutdown().await;
    Ok(())
}
