use std::path::Path;
use std::sync::Arc;

use super::ScriptRuntime;

pub fn create_runtime(path: &Path) -> anyhow::Result<Arc<dyn ScriptRuntime>> {
    let ext = path.extension().and_then(|s| s.to_str()).unwrap_or("");
    let script = std::fs::read_to_string(path)?;
    match ext {
        #[cfg(feature = "lua")]
        "lua" => Ok(Arc::new(super::lua::LuaRuntime::new(path, script)?)),
        _ => {
            anyhow::bail!("unsupported script extension `{ext}`: {}", path.display());
        }
    }
}
