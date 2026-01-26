#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(i32)]
pub enum ExitCode {
    Success = 0,

    /// One or more checks failed.
    ChecksFailed = 10,

    /// One or more thresholds failed.
    ThresholdsFailed = 11,

    /// Checks and thresholds failed.
    ChecksAndThresholdsFailed = 12,

    /// Script execution error (runtime raised an error while executing the user script).
    ScriptError = 20,

    /// Invalid CLI/config/options (bad flags, invalid durations, invalid thresholds syntax, etc.).
    InvalidInput = 30,

    /// Internal/runtime error (IO errors, unexpected invariants, panics caught at top-level).
    RuntimeError = 40,
}

impl ExitCode {
    #[must_use]
    pub fn as_i32(self) -> i32 {
        self as i32
    }

    #[must_use]
    pub fn from_quality_gates(checks_failed: bool, thresholds_failed: bool) -> Self {
        match (checks_failed, thresholds_failed) {
            (false, false) => Self::Success,
            (true, false) => Self::ChecksFailed,
            (false, true) => Self::ThresholdsFailed,
            (true, true) => Self::ChecksAndThresholdsFailed,
        }
    }
}
