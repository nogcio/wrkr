use clap::Parser;
use std::path::PathBuf;

#[derive(Debug, Parser)]
#[command(
    name = "wrkr-tools-compare-perf",
    about = "Cross-platform perf comparison runner for wrkr/wrk/k6"
)]
pub struct Cli {
    /// Root of the wrkr repository (defaults to current working directory)
    #[arg(long, env = "WRKR_ROOT")]
    pub(crate) root: Option<PathBuf>,

    /// Duration to run each case (e.g. 5s)
    #[arg(long, env = "DURATION", default_value = "5s")]
    pub(crate) duration: String,

    /// Build required binaries before running
    #[arg(long, default_value_t = true)]
    pub(crate) build: bool,

    /// Build with -C target-cpu=native (best perf, machine-specific)
    #[arg(long, env = "NATIVE", default_value_t = true)]
    pub(crate) native: bool,

    /// Number of virtual users for wrkr
    #[arg(long, env = "WRKR_VUS", default_value_t = 256)]
    pub(crate) wrkr_vus: u64,

    /// Number of VUs for k6
    #[arg(long, env = "K6_VUS")]
    pub(crate) k6_vus: Option<u64>,

    /// wrk threads
    #[arg(long, env = "WRK_THREADS", default_value_t = 8)]
    pub(crate) wrk_threads: u32,

    /// wrk connections
    #[arg(long, env = "WRK_CONNECTIONS", default_value_t = 256)]
    pub(crate) wrk_connections: u32,

    /// Gate: wrkr_rps must be >= wrk_rps * ratio
    #[arg(long, env = "RATIO_OK", default_value_t = 0.95)]
    pub(crate) ratio_ok_get_hello: f64,

    /// Gate: wrkr_rps must be >= wrk_rps * ratio
    #[arg(long, env = "RATIO_OK_POST_JSON", default_value_t = 0.90)]
    pub(crate) ratio_ok_post_json: f64,

    /// Gate: wrkr_rps must be > k6_rps * ratio
    #[arg(long, env = "RATIO_OK_WRKR_OVER_K6", default_value_t = 1.5)]
    pub(crate) ratio_ok_wrkr_over_k6: f64,

    /// Gate for gRPC: wrkr_rps must be > k6_rps * ratio
    #[arg(long, env = "RATIO_OK_GRPC_WRKR_OVER_K6", default_value_t = 1.5)]
    pub(crate) ratio_ok_grpc_wrkr_over_k6: f64,

    /// Optional cross-protocol gate: wrkr gRPC RPS must be >= wrk GET /hello RPS * ratio
    #[arg(long, env = "RATIO_OK_GRPC_WRKR_OVER_WRK_HELLO", default_value_t = 0.9)]
    pub(crate) ratio_ok_grpc_wrkr_over_wrk_hello: f64,

    /// If set, missing wrk is a hard error; otherwise HTTP wrk comparisons are skipped
    #[arg(long, default_value_t = false)]
    pub(crate) require_wrk: bool,

    /// If set, missing k6 is a hard error; otherwise k6 comparisons are skipped
    #[arg(long, default_value_t = false)]
    pub(crate) require_k6: bool,
}
