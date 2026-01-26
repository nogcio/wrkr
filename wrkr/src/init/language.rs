#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScriptLanguage {
    #[cfg(feature = "lua")]
    Lua,
}

impl ScriptLanguage {
    pub fn parse(s: &str) -> anyhow::Result<Self> {
        let s = s.trim();

        match s {
            #[cfg(feature = "lua")]
            "lua" => Ok(Self::Lua),
            _ => anyhow::bail!(
                "unsupported script language '{s}'. Available languages: {}",
                available_languages()
            ),
        }
    }
}

fn available_languages() -> &'static str {
    #[cfg(feature = "lua")]
    {
        "lua"
    }

    #[cfg(not(feature = "lua"))]
    {
        "(none - this build has no script runtimes enabled)"
    }
}
