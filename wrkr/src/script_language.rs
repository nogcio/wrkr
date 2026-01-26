#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScriptLanguage {
    #[cfg(feature = "lua")]
    Lua,

    // This variant exists only so the enum is non-empty when no script runtimes are compiled in.
    #[cfg(not(feature = "lua"))]
    _NoRuntimes,
}

pub fn parse_script_language(s: &str) -> Result<ScriptLanguage, String> {
    let s = s.trim();

    #[cfg(feature = "lua")]
    {
        match s {
            "lua" => Ok(ScriptLanguage::Lua),
            _ => Err(format!(
                "unsupported script language '{s}'. Available languages: {}",
                available_languages()
            )),
        }
    }

    #[cfg(not(feature = "lua"))]
    {
        let _ = s;
        Err("this build of wrkr has no script runtimes enabled".to_string())
    }
}

pub fn available_languages() -> &'static str {
    #[cfg(feature = "lua")]
    {
        "lua"
    }

    #[cfg(not(feature = "lua"))]
    {
        "(none)"
    }
}
