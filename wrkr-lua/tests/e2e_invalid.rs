mod support;

use wrkr_lua::Result;

#[tokio::test]
async fn invalid_metric_name_script_fails() -> Result<()> {
    let res = support::run_script(
        "invalid_metric_name.lua",
        &[],
        wrkr_core::RunConfig::default(),
    )
    .await;

    let err = match res {
        Ok(_) => panic!("expected script to fail"),
        Err(err) => err,
    };

    let msg = err.to_string();
    assert!(msg.contains("invalid metric name"), "{msg}");

    Ok(())
}
