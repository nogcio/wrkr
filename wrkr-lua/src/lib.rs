pub use wrkr_core::{RunConfig, ScenarioConfig, ScenarioOptions, ScriptOptions};

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("lua error: {0}")]
    Lua(#[from] mlua::Error),

    #[error("io error: {0}")]
    Io(#[from] std::io::Error),

    #[error("core error: {0}")]
    Core(#[from] wrkr_core::Error),

    #[error("expected function `Default()` in script")]
    MissingDefault,

    #[error("expected function `{0}()` in script")]
    MissingExec(String),

    #[error("script path missing; cannot resolve `{0}`")]
    MissingScriptPath(String),

    #[error("invalid script-relative path: `{0}`")]
    InvalidPath(String),

    #[error("`Options.iterations` must be a positive integer")]
    InvalidIterations,

    #[error("`Options.vus` must be a positive integer")]
    InvalidVus,

    #[error("`Options.scenarios[*].executor` must be a string")]
    InvalidExecutor,

    #[error("`Options.scenarios[*].stages` must be an array of {{ duration, target }}")]
    InvalidStages,

    #[error("`Options.duration` must be a valid duration, e.g. 10s, 250ms")]
    InvalidDuration,

    #[error("`Options.scenarios[*].time_unit` must be a valid duration, e.g. 1s")]
    InvalidTimeUnit,

    #[error("`Options.scenarios[*].tags` must be a table of string -> scalar")]
    InvalidScenarioTags,

    #[error("`Options.thresholds` must be a table of metric -> [expr, ...]")]
    InvalidThresholds,

    #[error("invalid metric name (expected non-empty string)")]
    InvalidMetricName,

    #[error("invalid metric value")]
    InvalidMetricValue,
}

mod debugger;
mod editor_stubs;
mod json_util;
mod lifecycle;
mod loader;
mod modules;
mod options;
mod value_util;
mod vu;

pub use editor_stubs::{StubFile, luals_stub_files};
pub use lifecycle::{run_handle_summary, run_setup, run_teardown};
pub use options::parse_script_options;
pub use vu::run_vu;
