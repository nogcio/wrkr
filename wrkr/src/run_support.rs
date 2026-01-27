use std::collections::BTreeMap;
use std::sync::Arc;

use anyhow::Context as _;

use crate::run_error::RunError;

pub(crate) fn merged_env(overrides: &[String]) -> anyhow::Result<wrkr_core::EnvVars> {
    let mut map: BTreeMap<String, String> = std::env::vars()
        .map(|(k, v)| (k.to_string(), v.to_string()))
        .collect();

    for raw in overrides {
        let (k, v) = parse_env_override(raw)?;
        map.insert(k, v);
    }

    let vars: Vec<(Arc<str>, Arc<str>)> = map
        .into_iter()
        .map(|(k, v)| (Arc::<str>::from(k), Arc::<str>::from(v)))
        .collect();

    Ok(Arc::from(vars.into_boxed_slice()))
}

fn parse_env_override(s: &str) -> anyhow::Result<(String, String)> {
    let (k, v) = s
        .split_once('=')
        .with_context(|| format!("invalid --env (expected KEY=VALUE): {s}"))?;
    if k.is_empty() {
        anyhow::bail!("invalid --env (empty KEY): {s}");
    }
    Ok((k.to_string(), v.to_string()))
}

pub(crate) fn classify_runtime_create_error(err: anyhow::Error) -> RunError {
    // Unsupported extensions and missing script files are treated as invalid input.
    if let Some(io) = err.downcast_ref::<std::io::Error>()
        && io.kind() == std::io::ErrorKind::NotFound
    {
        return RunError::InvalidInput(err);
    }
    RunError::InvalidInput(err)
}

pub(crate) fn classify_runtime_error(
    context: &'static str,
    err: crate::runtime::RuntimeError,
) -> RunError {
    #[cfg(feature = "lua")]
    {
        match err {
            crate::runtime::RuntimeError::Lua(lua_err) => {
                use wrkr_lua::Error as LuaError;

                let kind = match &lua_err {
                    // Invalid options/config input.
                    LuaError::InvalidIterations
                    | LuaError::InvalidVus
                    | LuaError::InvalidExecutor
                    | LuaError::InvalidStages
                    | LuaError::InvalidDuration
                    | LuaError::InvalidTimeUnit
                    | LuaError::InvalidScenarioTags
                    | LuaError::InvalidThresholds => RunError::InvalidInput,

                    // User script error (runtime error, missing entrypoints, bad API use).
                    LuaError::Lua(_)
                    | LuaError::MissingDefault
                    | LuaError::MissingExec(_)
                    | LuaError::MissingScriptPath(_)
                    | LuaError::InvalidPath(_)
                    | LuaError::InvalidMetricName
                    | LuaError::InvalidMetricValue => RunError::ScriptError,

                    // Core errors surfaced through the Lua layer.
                    LuaError::Core(_) => RunError::InvalidInput,

                    // IO while executing script hooks/modules.
                    LuaError::Io(_) => RunError::RuntimeError,
                };

                kind(anyhow::Error::new(lua_err).context(context))
            }
        }
    }

    #[cfg(not(feature = "lua"))]
    {
        let _ = context;
        RunError::RuntimeError(anyhow::Error::new(err))
    }
}
