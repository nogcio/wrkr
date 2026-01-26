use crate::exit_codes::ExitCode;

#[derive(Debug)]
pub enum RunError {
    InvalidInput(anyhow::Error),
    ScriptError(anyhow::Error),
    RuntimeError(anyhow::Error),
}

impl RunError {
    #[must_use]
    pub fn exit_code(&self) -> ExitCode {
        match self {
            Self::InvalidInput(_) => ExitCode::InvalidInput,
            Self::ScriptError(_) => ExitCode::ScriptError,
            Self::RuntimeError(_) => ExitCode::RuntimeError,
        }
    }

    #[must_use]
    pub fn anyhow(&self) -> &anyhow::Error {
        match self {
            Self::InvalidInput(e) | Self::ScriptError(e) | Self::RuntimeError(e) => e,
        }
    }
}

impl std::fmt::Display for RunError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidInput(e) | Self::ScriptError(e) | Self::RuntimeError(e) => {
                write!(f, "{e:#}")
            }
        }
    }
}

impl std::error::Error for RunError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(self.anyhow().as_ref())
    }
}
