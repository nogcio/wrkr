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
async fn http_opts_name_is_recorded_as_name_tag() -> Result<()> {
    let server = TestServer::start().await?;

    let script = support::load_test_script("http_name.lua")?;
    let env = support::env_with(&[("BASE_URL", server.base_url().to_string())]);
    let run_ctx = support::run_ctx_for_script(&script, env);

    let opts = wrkr_lua::parse_script_options(&run_ctx)?;
    let scenarios = wrkr_core::scenarios_from_options(opts, wrkr_core::RunConfig::default())?;

    let run_ctx_after = run_ctx.clone();
    let _summary = wrkr_core::run_scenarios(scenarios, run_ctx, wrkr_lua::run_vu, None).await?;

    let series = run_ctx_after.metrics.summarize();

    series
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
                        ("group", "g_http"),
                        ("name", "GET /plaintext"),
                        ("route", "/plaintext"),
                    ],
                )
        })
        .unwrap_or_else(|| panic!("missing requests_total series with expected name tag"));

    server.shutdown().await;
    Ok(())
}
