#[derive(Debug)]
pub enum RuntimeError {
    #[cfg(feature = "lua")]
    Lua(wrkr_lua::Error),

    #[cfg(not(feature = "lua"))]
    Unavailable(&'static str),
}

impl std::fmt::Display for RuntimeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            #[cfg(feature = "lua")]
            Self::Lua(e) => write!(f, "{e}"),
            #[cfg(not(feature = "lua"))]
            Self::Unavailable(msg) => write!(f, "{msg}"),
        }
    }
}

impl std::error::Error for RuntimeError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            #[cfg(feature = "lua")]
            Self::Lua(e) => Some(e),
            #[cfg(not(feature = "lua"))]
            Self::Unavailable(_) => None,
        }
    }
}

#[cfg(feature = "lua")]
impl From<wrkr_lua::Error> for RuntimeError {
    fn from(value: wrkr_lua::Error) -> Self {
        Self::Lua(value)
    }
}
