mod support;

use wrkr_lua::Result;

#[tokio::test]
async fn e2e_uuid_script_runs() -> Result<()> {
    support::run_script("uuid.lua", &[], wrkr_core::RunConfig::default()).await?;
    Ok(())
}

#[tokio::test]
async fn e2e_shared_store_script_runs() -> Result<()> {
    support::run_script("shared_store.lua", &[], wrkr_core::RunConfig::default()).await?;
    Ok(())
}
