mod support;

#[test]
fn parse_script_options_rejects_invalid_duration() {
    let script = support::load_test_script("options_invalid_duration.lua")
        .unwrap_or_else(|err| panic!("load script: {err}"));
    let env = support::env_with(&[]);
    let run_ctx = support::run_ctx_for_script(&script, env);

    let err = match wrkr_lua::parse_script_options(&run_ctx) {
        Ok(_) => panic!("expected parse_script_options to fail"),
        Err(err) => err,
    };
    let msg = err.to_string();
    assert!(
        msg.contains("InvalidDuration") || msg.contains("options.duration"),
        "{msg}"
    );
}

#[test]
fn parse_script_options_rejects_invalid_thresholds() {
    let script = support::load_test_script("options_invalid_thresholds.lua")
        .unwrap_or_else(|err| panic!("load script: {err}"));
    let env = support::env_with(&[]);
    let run_ctx = support::run_ctx_for_script(&script, env);

    let err = match wrkr_lua::parse_script_options(&run_ctx) {
        Ok(_) => panic!("expected parse_script_options to fail"),
        Err(err) => err,
    };
    let msg = err.to_string();
    assert!(
        msg.contains("InvalidThresholds") || msg.contains("options.thresholds"),
        "{msg}"
    );
}
