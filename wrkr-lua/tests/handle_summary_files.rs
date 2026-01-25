mod support;

use std::path::PathBuf;

use wrkr_lua::Result;

fn temp_dir(name: &str) -> PathBuf {
    let id = uuid::Uuid::new_v4();
    std::env::temp_dir().join(format!("wrkr-lua-{name}-{id}"))
}

#[test]
fn handle_summary_invalid_path_is_rejected_by_write_output_files() -> Result<()> {
    let script = support::load_test_script("handle_summary_invalid_path.lua")?;
    let env = support::env_with(&[]);
    let run_ctx = support::run_ctx_for_script(&script, env);

    let out = wrkr_lua::run_handle_summary(&run_ctx)?;
    let Some(out) = out else {
        panic!("expected HandleSummary outputs");
    };

    let base = temp_dir("outputs");
    std::fs::create_dir_all(&base).unwrap_or_else(|err| panic!("create temp dir: {err}"));

    let res = wrkr_core::write_output_files(&base, &out.files);
    let err = match res {
        Ok(_) => panic!("expected write_output_files to fail"),
        Err(err) => err,
    };

    let msg = err.to_string();
    assert!(
        msg.contains("InvalidOutputPath") || msg.contains("invalid output"),
        "{msg}"
    );

    Ok(())
}

#[tokio::test]
async fn handle_summary_checks_by_name_produces_json_file() -> Result<()> {
    support::run_script(
        "handle_summary_checks_by_name.lua",
        &[],
        wrkr_core::RunConfig::default(),
    )
    .await?;

    let script = support::load_test_script("handle_summary_checks_by_name.lua")?;
    let env = support::env_with(&[]);
    let run_ctx = support::run_ctx_for_script(&script, env);

    let out = wrkr_lua::run_handle_summary(&run_ctx)?;
    let Some(out) = out else {
        panic!("expected HandleSummary outputs");
    };

    let base = temp_dir("summary");
    std::fs::create_dir_all(&base).unwrap_or_else(|err| panic!("create temp dir: {err}"));
    wrkr_core::write_output_files(&base, &out.files)?;

    let json_path = base.join("summary.json");
    let raw = std::fs::read_to_string(&json_path)
        .unwrap_or_else(|err| panic!("read summary.json: {err}"));
    let v: serde_json::Value = serde_json::from_str(raw.trim_end())
        .unwrap_or_else(|err| panic!("parse summary json: {err}"));

    assert!(v.is_object(), "expected summary to be a JSON object");

    Ok(())
}
