mod support;

use std::time::Duration;

use wrkr_lua::Result;

#[test]
fn parse_script_options_duration_in_scenarios() -> Result<()> {
    let script = support::load_test_script("duration.lua")?;
    let env = support::env_with(&[("BASE_URL", "http://127.0.0.1".to_string())]);
    let run_ctx = support::run_ctx_for_script(&script, env);

    let opts = wrkr_lua::parse_script_options(&run_ctx)?;
    assert!(opts.duration.is_none());
    assert_eq!(opts.scenarios.len(), 1);

    let s = &opts.scenarios[0];
    assert_eq!(s.name, "main");
    assert_eq!(s.exec.as_deref(), Some("Default"));
    assert_eq!(s.vus, Some(2));
    assert_eq!(s.duration, Some(Duration::from_millis(50)));

    Ok(())
}

#[test]
fn parse_script_options_thresholds_table() -> Result<()> {
    let script = support::load_test_script("thresholds.lua")?;
    let env = support::env_with(&[("BASE_URL", "http://127.0.0.1".to_string())]);
    let run_ctx = support::run_ctx_for_script(&script, env);

    let opts = wrkr_lua::parse_script_options(&run_ctx)?;
    assert_eq!(opts.thresholds.len(), 1);
    assert_eq!(opts.thresholds[0].metric, "http_req_duration");
    assert_eq!(opts.thresholds[0].expressions, vec!["avg<0".to_string()]);

    Ok(())
}

#[test]
fn parse_script_options_accepts_camel_and_snake_case_aliases() -> Result<()> {
    let script = support::load_test_script("options_aliases_ramping_vus.lua")?;
    let env = support::env_with(&[]);
    let run_ctx = support::run_ctx_for_script(&script, env);

    let opts = wrkr_lua::parse_script_options(&run_ctx)?;
    assert_eq!(opts.scenarios.len(), 2);

    let mut by_name = opts
        .scenarios
        .iter()
        .map(|s| (s.name.as_str(), s))
        .collect::<std::collections::BTreeMap<_, _>>();

    let camel = by_name
        .remove("camel")
        .unwrap_or_else(|| panic!("missing 'camel' scenario"));
    assert_eq!(camel.executor.as_deref(), Some("ramping-vus"));
    assert_eq!(camel.start_vus, Some(3));
    assert_eq!(camel.stages.len(), 1);
    assert_eq!(camel.stages[0].target, 5);

    let snake = by_name
        .remove("snake")
        .unwrap_or_else(|| panic!("missing 'snake' scenario"));
    assert_eq!(snake.executor.as_deref(), Some("ramping-vus"));
    assert_eq!(snake.start_vus, Some(4));
    assert_eq!(snake.stages.len(), 1);
    assert_eq!(snake.stages[0].target, 6);

    Ok(())
}

#[test]
fn parse_script_options_arrival_rate_aliases() -> Result<()> {
    let script = support::load_test_script("options_aliases_arrival_rate.lua")?;
    let env = support::env_with(&[]);
    let run_ctx = support::run_ctx_for_script(&script, env);

    let opts = wrkr_lua::parse_script_options(&run_ctx)?;
    assert_eq!(opts.scenarios.len(), 2);

    let mut by_name = opts
        .scenarios
        .iter()
        .map(|s| (s.name.as_str(), s))
        .collect::<std::collections::BTreeMap<_, _>>();

    let camel = by_name
        .remove("camel")
        .unwrap_or_else(|| panic!("missing 'camel' scenario"));
    assert_eq!(camel.executor.as_deref(), Some("ramping-arrival-rate"));
    assert_eq!(camel.start_rate, Some(123));
    assert_eq!(camel.time_unit, Some(Duration::from_millis(250)));
    assert_eq!(camel.pre_allocated_vus, Some(7));
    assert_eq!(camel.max_vus, Some(99));
    assert_eq!(camel.stages.len(), 2);

    let snake = by_name
        .remove("snake")
        .unwrap_or_else(|| panic!("missing 'snake' scenario"));
    assert_eq!(snake.executor.as_deref(), Some("ramping-arrival-rate"));
    assert_eq!(snake.start_rate, Some(321));
    assert_eq!(snake.time_unit, Some(Duration::from_secs(2)));
    assert_eq!(snake.pre_allocated_vus, Some(11));
    assert_eq!(snake.max_vus, Some(22));
    assert_eq!(snake.stages.len(), 1);
    assert_eq!(snake.stages[0].target, 33);

    Ok(())
}

#[test]
fn parse_script_options_scenario_tags() -> Result<()> {
    let script = support::load_test_script("scenario_tags.lua")?;
    let env = support::env_with(&[]);
    let run_ctx = support::run_ctx_for_script(&script, env);

    let opts = wrkr_lua::parse_script_options(&run_ctx)?;
    assert_eq!(opts.scenarios.len(), 1);

    let s = &opts.scenarios[0];
    assert_eq!(s.name, "main");

    let mut tags = s
        .tags
        .iter()
        .map(|(k, v)| (k.as_str(), v.as_str()))
        .collect::<std::collections::BTreeMap<_, _>>();

    assert_eq!(tags.remove("env"), Some("staging"));
    assert_eq!(tags.remove("build"), Some("123"));
    assert_eq!(tags.remove("ok"), Some("true"));
    assert_eq!(
        tags.remove("group"),
        Some("should_not_override_runtime_group")
    );

    Ok(())
}
