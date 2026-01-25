mod support;

use wrkr_lua::Result;
use wrkr_testserver::TestServer;

fn tags_get<'a>(tags: &'a [(String, String)], key: &str) -> Option<&'a str> {
    tags.iter()
        .find_map(|(k, v)| (k == key).then_some(v.as_str()))
}

fn tags_contain_all(tags: &[(String, String)], expected: &[(&str, &str)]) -> bool {
    expected.iter().all(|(k, v)| tags_get(tags, k) == Some(*v))
}

#[tokio::test]
async fn scenario_tags_group_checks_and_http_metrics_are_correct() -> Result<()> {
    let server = TestServer::start().await?;

    let script = support::load_test_script("scenario_tags_checks_http.lua")?;
    let env = support::env_with(&[("BASE_URL", server.base_url().to_string())]);
    let run_ctx = support::run_ctx_for_script(&script, env);

    let opts = wrkr_lua::parse_script_options(&run_ctx)?;
    let scenarios = wrkr_core::scenarios_from_options(opts, wrkr_core::RunConfig::default())?;

    let run_ctx_after = run_ctx.clone();
    let _summary = wrkr_core::run_scenarios(scenarios, run_ctx, wrkr_lua::run_vu, None).await?;

    let series = run_ctx_after.metrics.summarize();

    // Checks should include scenario tags + runtime group.
    let checks = series
        .iter()
        .find(|m| {
            m.name == "checks"
                && tags_contain_all(
                    &m.tags,
                    &[
                        ("scenario", "main"),
                        ("env", "staging"),
                        ("build", "123"),
                        ("ok", "true"),
                        ("group", "g_runtime"),
                        ("name", "http_ok"),
                        ("status", "pass"),
                    ],
                )
        })
        .unwrap_or_else(|| panic!("missing checks series with scenario tags and runtime group"));
    assert_eq!(tags_get(&checks.tags, "group"), Some("g_runtime"));

    // HTTP request metrics should include scenario tags + runtime group + request tags.
    // (requests_total is recorded per (scenario, protocol) plus extra tags)
    let http_requests = series
        .iter()
        .find(|m| {
            m.name == "requests_total"
                && tags_contain_all(
                    &m.tags,
                    &[
                        ("scenario", "main"),
                        ("protocol", "http"),
                        ("env", "staging"),
                        ("build", "123"),
                        ("ok", "true"),
                        ("group", "g_runtime"),
                        ("route", "/plaintext"),
                    ],
                )
        })
        .unwrap_or_else(|| panic!("missing requests_total series with expected tags"));
    assert_eq!(tags_get(&http_requests.tags, "group"), Some("g_runtime"));

    server.shutdown().await;
    Ok(())
}
