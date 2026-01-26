mod support;

use wrkr_lua::Result;

#[tokio::test]
async fn check_accepts_any_value_and_records_failures() -> Result<()> {
    // Intentionally fail a check so we can assert it was recorded.
    let summary =
        support::run_script("check_any_value.lua", &[], wrkr_core::RunConfig::default()).await?;

    assert_eq!(summary.scenarios.len(), 1);

    let s = &summary.scenarios[0];
    assert_eq!(s.iterations_total, 1);
    assert_eq!(s.checks_failed_total, 1);
    assert_eq!(s.checks_failed.get("is hello"), Some(&1));

    Ok(())
}
