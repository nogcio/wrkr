use crate::script_language::ScriptLanguage;

#[cfg(feature = "lua")]
mod fs;

#[cfg(feature = "lua")]
mod lua;

use anyhow::Context as _;

use crate::cli::InitArgs;

pub async fn init(args: InitArgs) -> anyhow::Result<()> {
    let root = &args.dir;
    tokio::fs::create_dir_all(root)
        .await
        .with_context(|| format!("failed to create dir: {}", root.display()))?;

    let lang = args.lang;

    match lang {
        #[cfg(feature = "lua")]
        ScriptLanguage::Lua => lua::scaffold(root, &args).await,

        #[cfg(not(feature = "lua"))]
        ScriptLanguage::_NoRuntimes => {
            anyhow::bail!("this build of wrkr has no script runtimes enabled")
        }
    }
}
