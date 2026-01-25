mod support;

use wrkr_lua::Result;

fn tags_get<'a>(tags: &'a [(String, String)], key: &str) -> Option<&'a str> {
    tags.iter()
        .find_map(|(k, v)| (k == key).then_some(v.as_str()))
}

fn tags_contain_all(tags: &[(String, String)], expected: &[(&str, &str)]) -> bool {
    expected.iter().all(|(k, v)| tags_get(tags, k) == Some(*v))
}

#[tokio::test]
async fn scenario_tags_are_merged_and_group_not_overridden() -> Result<()> {
    let script = support::load_test_script("scenario_tags_metrics.lua")?;
    let env = support::env_with(&[]);
    let run_ctx = support::run_ctx_for_script(&script, env);

    let opts = wrkr_lua::parse_script_options(&run_ctx)?;
    let scenarios = wrkr_core::scenarios_from_options(opts, wrkr_core::RunConfig::default())?;

    let run_ctx_after = run_ctx.clone();
    let _summary = wrkr_core::run_scenarios(scenarios, run_ctx, wrkr_lua::run_vu, None).await?;

    let series = run_ctx_after.metrics.summarize();

    series
        .iter()
        .find(|m| {
            m.name == "checks"
                && tags_contain_all(
                    &m.tags,
                    &[
                        ("scenario", "main"),
                        ("env", "staging"),
                        ("build", "123"),
                        ("group", "g_runtime"),
                        ("name", "ok"),
                        ("status", "pass"),
                    ],
                )
        })
        .unwrap_or_else(|| panic!("missing checks series with scenario tags and runtime group"));

    series
        .iter()
        .find(|m| {
            m.name == "custom_counter_scenario_tags"
                && tags_contain_all(
                    &m.tags,
                    &[
                        ("scenario", "main"),
                        ("env", "staging"),
                        ("build", "123"),
                        ("group", "g_runtime"),
                        ("k", "v"),
                    ],
                )
        })
        .unwrap_or_else(|| {
            panic!(
                "missing custom_counter_scenario_tags series with scenario tags and runtime group"
            )
        });

    Ok(())
}
