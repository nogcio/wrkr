use std::time::Duration;

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, strum::Display, strum::EnumString)]
#[strum(serialize_all = "snake_case")]
pub enum HttpTransportErrorKind {
    InvalidUrl,
    OnlyHttpSupported,
    RequestBuild,
    HeaderName,
    HeaderValue,
    Request,
    Timeout,
    BodyRead,
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("invalid url: {0}")]
    InvalidUrl(String),

    #[error("only http:// URLs are supported for now: {0}")]
    OnlyHttpSupported(String),

    #[error("http request build failed: {0}")]
    RequestBuild(#[from] http::Error),

    #[error("invalid http header name: {0}")]
    HeaderName(#[from] http::header::InvalidHeaderName),

    #[error("invalid http header value: {0}")]
    HeaderValue(#[from] http::header::InvalidHeaderValue),

    #[error("http request failed: {0}")]
    Request(#[from] hyper_util::client::legacy::Error),

    #[error("http request timed out after {0:?}")]
    Timeout(Duration),

    #[error("failed to read response body: {0}")]
    BodyRead(#[from] hyper::Error),
}

impl Error {
    #[must_use]
    pub fn transport_error_kind(&self) -> HttpTransportErrorKind {
        match self {
            Self::InvalidUrl(_) => HttpTransportErrorKind::InvalidUrl,
            Self::OnlyHttpSupported(_) => HttpTransportErrorKind::OnlyHttpSupported,
            Self::RequestBuild(_) => HttpTransportErrorKind::RequestBuild,
            Self::HeaderName(_) => HttpTransportErrorKind::HeaderName,
            Self::HeaderValue(_) => HttpTransportErrorKind::HeaderValue,
            Self::Request(_) => HttpTransportErrorKind::Request,
            Self::Timeout(_) => HttpTransportErrorKind::Timeout,
            Self::BodyRead(_) => HttpTransportErrorKind::BodyRead,
        }
    }
}
