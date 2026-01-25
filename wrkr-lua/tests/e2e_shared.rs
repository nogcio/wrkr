mod support;

use wrkr_lua::Result;

#[tokio::test]
async fn e2e_shared_store_synchronizes_across_vus() -> Result<()> {
    // Script defines its own multi-VU scenario; avoid CLI overrides.
    support::run_script(
        "shared_store_multi_vu.lua",
        &[],
        wrkr_core::RunConfig::default(),
    )
    .await?;

    Ok(())
}
