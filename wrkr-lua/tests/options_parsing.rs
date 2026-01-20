use std::path::Path;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

fn read_test_script(name: &str) -> (PathBuf, String) {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/scripts")
        .join(name);

    let script = match std::fs::read_to_string(&path) {
        Ok(v) => v,
        Err(err) => panic!("failed to read lua script {}: {err}", path.display()),
    };

    (path, script)
}

fn parse_options(script_path: &Path, script: &str) -> wrkr_core::runner::ScriptOptions {
    let env: wrkr_core::runner::EnvVars =
        Arc::from(Vec::<(Arc<str>, Arc<str>)>::new().into_boxed_slice());
    let options_client = Arc::new(wrkr_core::HttpClient::default());
    let options_stats = Arc::new(wrkr_core::runner::RunStats::default());
    let shared = Arc::new(wrkr_core::runner::SharedStore::default());

    match wrkr_lua::parse_script_options(
        script,
        Some(script_path),
        &env,
        options_client,
        options_stats,
        shared,
    ) {
        Ok(v) => v,
        Err(err) => panic!("parse_script_options failed: {err}"),
    }
}

#[test]
fn options_parsing_accepts_arrival_rate_aliases() {
    let (script_path, script) = read_test_script("options_aliases_arrival_rate.lua");
    let opts = parse_options(&script_path, &script);

    assert_eq!(opts.scenarios.len(), 2);

    let camel = opts
        .scenarios
        .iter()
        .find(|s| s.name == "camel")
        .unwrap_or_else(|| panic!("missing scenario: camel"));
    assert_eq!(camel.executor.as_deref(), Some("ramping-arrival-rate"));
    assert_eq!(camel.exec.as_deref(), Some("Default"));
    assert_eq!(camel.start_rate, Some(123));
    assert_eq!(camel.time_unit, Some(Duration::from_millis(250)));
    assert_eq!(camel.pre_allocated_vus, Some(7));
    assert_eq!(camel.max_vus, Some(99));
    assert_eq!(camel.stages.len(), 2);
    assert_eq!(camel.stages[0].duration, Duration::from_secs(1));
    assert_eq!(camel.stages[0].target, 1000);
    assert_eq!(camel.stages[1].duration, Duration::from_secs(2));
    assert_eq!(camel.stages[1].target, 10);

    let snake = opts
        .scenarios
        .iter()
        .find(|s| s.name == "snake")
        .unwrap_or_else(|| panic!("missing scenario: snake"));
    assert_eq!(snake.executor.as_deref(), Some("ramping-arrival-rate"));
    assert_eq!(snake.exec.as_deref(), Some("Default"));
    assert_eq!(snake.start_rate, Some(321));
    assert_eq!(snake.time_unit, Some(Duration::from_secs(2)));
    assert_eq!(snake.pre_allocated_vus, Some(11));
    assert_eq!(snake.max_vus, Some(22));
    assert_eq!(snake.stages.len(), 1);
    assert_eq!(snake.stages[0].duration, Duration::from_secs(3));
    assert_eq!(snake.stages[0].target, 33);
}

#[test]
fn options_parsing_accepts_ramping_vus_aliases() {
    let (script_path, script) = read_test_script("options_aliases_ramping_vus.lua");
    let opts = parse_options(&script_path, &script);

    assert_eq!(opts.scenarios.len(), 2);

    let camel = opts
        .scenarios
        .iter()
        .find(|s| s.name == "camel")
        .unwrap_or_else(|| panic!("missing scenario: camel"));
    assert_eq!(camel.executor.as_deref(), Some("ramping-vus"));
    assert_eq!(camel.exec.as_deref(), Some("Default"));
    assert_eq!(camel.start_vus, Some(3));
    assert_eq!(camel.stages.len(), 1);
    assert_eq!(camel.stages[0].duration, Duration::from_secs(1));
    assert_eq!(camel.stages[0].target, 5);

    let snake = opts
        .scenarios
        .iter()
        .find(|s| s.name == "snake")
        .unwrap_or_else(|| panic!("missing scenario: snake"));
    assert_eq!(snake.executor.as_deref(), Some("ramping-vus"));
    assert_eq!(snake.exec.as_deref(), Some("Default"));
    assert_eq!(snake.start_vus, Some(4));
    assert_eq!(snake.stages.len(), 1);
    assert_eq!(snake.stages[0].duration, Duration::from_secs(1));
    assert_eq!(snake.stages[0].target, 6);
}
