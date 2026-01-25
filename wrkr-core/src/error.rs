pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),

    #[error("task join error: {0}")]
    Join(#[from] tokio::task::JoinError),

    #[error("virtual user error: {0}")]
    Vu(String),

    #[error("`vus` must be a positive integer")]
    InvalidVus,

    #[error("`iterations` must be a positive integer")]
    InvalidIterations,

    #[error(
        "invalid `executor` (expected `constant-vus`, `ramping-vus`, or `ramping-arrival-rate`)"
    )]
    InvalidExecutor,

    #[error("`stages` must be a non-empty array of {{ duration, target }}")]
    InvalidStages,

    #[error("`start_vus` must be a positive integer")]
    InvalidStartVus,

    #[error("`start_rate` must be a positive integer")]
    InvalidStartRate,

    #[error("`time_unit` must be a positive duration")]
    InvalidTimeUnit,

    #[error("`pre_allocated_vus` must be a positive integer")]
    InvalidPreAllocatedVus,

    #[error("`max_vus` must be >= `pre_allocated_vus`")]
    InvalidMaxVus,

    #[error("invalid output path: `{0}`")]
    InvalidOutputPath(String),
}
