use std::process::ExitStatus;

type Bytes = u64;

pub(crate) type RssBytes = Bytes;

#[derive(Debug, Clone)]
pub(crate) struct RunResult {
    pub(crate) status: ExitStatus,
    pub(crate) stdout: String,
    pub(crate) stderr: String,
    pub(crate) peak_rss_bytes: RssBytes,
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct Rps(pub(crate) f64);

#[derive(Debug, Clone, Copy)]
pub(crate) struct Mb(pub(crate) f64);

impl Mb {
    pub(crate) fn from_bytes(bytes: RssBytes) -> Self {
        Self((bytes as f64) / 1024.0 / 1024.0)
    }
}
