mod support;

use wrkr_lua::Result;

#[test]
fn run_setup_ignores_missing_or_nil_setup() -> Result<()> {
    let script = support::load_test_script("setup_nil.lua")?;
    let env = support::env_with(&[]);
    let run_ctx = support::run_ctx_for_script(&script, env);

    wrkr_lua::run_setup(&run_ctx)?;
    Ok(())
}

#[test]
fn run_teardown_ignores_missing_teardown() -> Result<()> {
    let script = support::load_test_script("plaintext.lua")?;
    let env = support::env_with(&[("BASE_URL", "http://127.0.0.1".to_string())]);
    let run_ctx = support::run_ctx_for_script(&script, env);

    wrkr_lua::run_teardown(&run_ctx)?;
    Ok(())
}

#[test]
fn handle_summary_outputs_are_parsed() -> Result<()> {
    let script = support::load_test_script("handle_summary_outputs.lua")?;
    let env = support::env_with(&[]);
    let run_ctx = support::run_ctx_for_script(&script, env);

    let out = wrkr_lua::run_handle_summary(&run_ctx)?;
    let Some(out) = out else {
        panic!("expected HandleSummary outputs");
    };

    assert_eq!(out.stdout.as_deref(), Some("hello\n"));
    assert_eq!(out.stderr.as_deref(), Some("oops\n"));

    assert_eq!(out.files.len(), 1);
    assert_eq!(out.files[0].0, "dir/out.txt");
    assert_eq!(out.files[0].1, "ok\n");

    Ok(())
}

#[test]
fn handle_summary_non_table_is_treated_as_empty_outputs() -> Result<()> {
    let script = support::load_test_script("handle_summary_non_table.lua")?;
    let env = support::env_with(&[]);
    let run_ctx = support::run_ctx_for_script(&script, env);

    let out = wrkr_lua::run_handle_summary(&run_ctx)?;
    let Some(out) = out else {
        panic!("expected HandleSummary outputs");
    };

    assert!(out.stdout.is_none());
    assert!(out.stderr.is_none());
    assert!(out.files.is_empty());

    Ok(())
}
