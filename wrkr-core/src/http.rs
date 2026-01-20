use bytes::Bytes;
use http_body_util::{BodyExt as _, Full};
use hyper::Request;
use hyper::body::Incoming;
use hyper_util::client::legacy::Client;
use hyper_util::client::legacy::connect::HttpConnector;
use hyper_util::rt::TokioExecutor;
use std::borrow::Cow;
use std::time::Duration;

pub type Result<T> = std::result::Result<T, Error>;

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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HttpResponse {
    pub status: u16,
    pub body: Bytes,
    /// Estimated bytes sent on the wire for this request (HTTP/1.1 request line + headers + body).
    pub bytes_sent: u64,
    /// Estimated bytes received on the wire for this response (HTTP/1.1 status line + headers + body).
    pub bytes_received: u64,
}

impl HttpResponse {
    pub fn body_utf8(&self) -> Option<&str> {
        std::str::from_utf8(&self.body).ok()
    }
}

#[derive(Debug, Clone)]
pub struct HttpClient {
    inner: Client<HttpConnector, Full<Bytes>>,
}

impl Default for HttpClient {
    fn default() -> Self {
        let mut connector = HttpConnector::new();
        connector.enforce_http(false);

        let inner = Client::builder(TokioExecutor::new()).build(connector);

        Self { inner }
    }
}

impl HttpClient {
    pub async fn request(&self, req: HttpRequest) -> Result<HttpResponse> {
        let timeout = req.timeout;
        let parsed = url::Url::parse(&req.url).map_err(|_| Error::InvalidUrl(req.url.clone()))?;
        if parsed.scheme() != "http" {
            return Err(Error::OnlyHttpSupported(req.url));
        }

        let bytes_sent = estimate_http_request_bytes_parts(
            &req.method,
            &req.url,
            &req.headers,
            req.body.len() as u64,
        )?;

        let uri: hyper::Uri = req
            .url
            .parse()
            .map_err(|_| Error::InvalidUrl(req.url.to_string()))?;

        let mut builder = Request::builder().method(req.method).uri(uri);

        // Make implicit headers explicit so our byte accounting is deterministic.
        // Note: we only support HTTP right now, so Host is always required.
        if !has_header(&req.headers, "host")
            && let Some(host) = host_header_value(&parsed)
        {
            builder = builder.header(http::header::HOST, host);
        }
        if !req.body.is_empty() && !has_header(&req.headers, "content-length") {
            builder = builder.header(http::header::CONTENT_LENGTH, req.body.len());
        }

        for (k, v) in req.headers {
            let name = http::header::HeaderName::from_bytes(k.as_bytes())?;
            let value = http::header::HeaderValue::from_str(&v)?;
            builder = builder.header(name, value);
        }

        let req: Request<Full<Bytes>> = builder.body(Full::new(req.body))?;

        let res: hyper::Response<Incoming> = if let Some(timeout) = timeout {
            match tokio::time::timeout(timeout, self.inner.request(req)).await {
                Ok(res) => res?,
                Err(_) => return Err(Error::Timeout(timeout)),
            }
        } else {
            self.inner.request(req).await?
        };

        let (parts, body) = res.into_parts();
        let status = parts.status.as_u16();
        let head_bytes =
            estimate_http1_response_head_bytes(parts.version, parts.status, &parts.headers);
        let body = body.collect().await?.to_bytes();
        let bytes_received = head_bytes.saturating_add(body.len() as u64);

        Ok(HttpResponse {
            status,
            body,
            bytes_sent,
            bytes_received,
        })
    }

    pub async fn get(&self, url: &str) -> Result<HttpResponse> {
        self.request(HttpRequest::get(url)).await
    }
}

/// Estimate bytes sent for an HTTP request.
///
/// This is a best-effort estimate of HTTP/1.1 framing: request line + headers + CRLF + body.
/// It intentionally makes Host/Content-Length explicit (if they are missing) for determinism.
pub fn estimate_http_request_bytes(req: &HttpRequest) -> Result<u64> {
    estimate_http_request_bytes_parts(&req.method, &req.url, &req.headers, req.body.len() as u64)
}

fn estimate_http_request_bytes_parts(
    method: &http::Method,
    url: &str,
    headers: &[(String, String)],
    body_len: u64,
) -> Result<u64> {
    let parsed = url::Url::parse(url).map_err(|_| Error::InvalidUrl(url.to_string()))?;
    if parsed.scheme() != "http" {
        return Err(Error::OnlyHttpSupported(url.to_string()));
    }

    let uri: hyper::Uri = url
        .parse()
        .map_err(|_| Error::InvalidUrl(url.to_string()))?;

    let mut bytes = 0u64;
    bytes = bytes.saturating_add(estimate_http1_request_line_bytes(method, &uri));

    // Headers (original + implicit ones we may add).
    for (k, v) in headers {
        bytes = bytes.saturating_add(estimate_http1_header_bytes(k.as_bytes(), v.as_bytes()));
    }

    if !has_header(headers, "host")
        && let Some(host) = host_header_value(&parsed)
    {
        bytes = bytes.saturating_add(estimate_http1_header_bytes(b"host", host.as_bytes()));
    }

    if body_len != 0 && !has_header(headers, "content-length") {
        let v = body_len.to_string();
        bytes = bytes.saturating_add(estimate_http1_header_bytes(b"content-length", v.as_bytes()));
    }

    // End of headers.
    bytes = bytes.saturating_add(2);
    bytes = bytes.saturating_add(body_len);
    Ok(bytes)
}

fn estimate_http1_request_line_bytes(method: &http::Method, uri: &hyper::Uri) -> u64 {
    let method_len = method.as_str().len() as u64;
    let path = uri.path_and_query().map(|p| p.as_str()).unwrap_or("/");
    let path_len = path.len() as u64;
    let version_len = "HTTP/1.1".len() as u64;

    // "METHOD SP path SP HTTP/1.1 CRLF"
    method_len
        .saturating_add(1)
        .saturating_add(path_len)
        .saturating_add(1)
        .saturating_add(version_len)
        .saturating_add(2)
}

fn estimate_http1_response_head_bytes(
    version: http::Version,
    status: http::StatusCode,
    headers: &http::HeaderMap,
) -> u64 {
    let mut bytes = 0u64;
    bytes = bytes.saturating_add(estimate_http1_status_line_bytes(version, status));
    for (name, value) in headers.iter() {
        bytes = bytes.saturating_add(estimate_http1_header_bytes(
            name.as_str().as_bytes(),
            value.as_bytes(),
        ));
    }
    bytes.saturating_add(2)
}

fn estimate_http1_status_line_bytes(version: http::Version, status: http::StatusCode) -> u64 {
    let version_str: Cow<'static, str> = match version {
        http::Version::HTTP_10 => Cow::Borrowed("HTTP/1.0"),
        http::Version::HTTP_11 => Cow::Borrowed("HTTP/1.1"),
        http::Version::HTTP_2 => Cow::Borrowed("HTTP/2"),
        http::Version::HTTP_3 => Cow::Borrowed("HTTP/3"),
        _ => Cow::Borrowed("HTTP/1.1"),
    };

    // "HTTP/1.1 SP 200 CRLF" (we intentionally ignore reason-phrase)
    (version_str.len() as u64)
        .saturating_add(1)
        .saturating_add(status.as_str().len() as u64)
        .saturating_add(2)
}

fn estimate_http1_header_bytes(name: &[u8], value: &[u8]) -> u64 {
    // "name: value\r\n"
    (name.len() as u64)
        .saturating_add(2)
        .saturating_add(value.len() as u64)
        .saturating_add(2)
}

fn has_header(headers: &[(String, String)], name: &str) -> bool {
    headers.iter().any(|(k, _)| k.eq_ignore_ascii_case(name))
}

fn host_header_value(parsed: &url::Url) -> Option<String> {
    let host = parsed.host_str()?;
    match parsed.port() {
        Some(port) if port != 80 => Some(format!("{host}:{port}")),
        _ => Some(host.to_string()),
    }
}

#[derive(Debug, Clone)]
pub struct HttpRequest {
    pub method: http::Method,
    pub url: String,
    pub headers: Vec<(String, String)>,
    pub body: Bytes,
    pub timeout: Option<Duration>,
}

impl HttpRequest {
    pub fn get(url: &str) -> Self {
        Self {
            method: http::Method::GET,
            url: url.to_string(),
            headers: Vec::new(),
            body: Bytes::new(),
            timeout: None,
        }
    }

    pub fn get_owned(url: String) -> Self {
        Self {
            method: http::Method::GET,
            url,
            headers: Vec::new(),
            body: Bytes::new(),
            timeout: None,
        }
    }

    pub fn post(url: &str, body: Bytes) -> Self {
        Self {
            method: http::Method::POST,
            url: url.to_string(),
            headers: Vec::new(),
            body,
            timeout: None,
        }
    }

    pub fn post_owned(url: String, body: Bytes) -> Self {
        Self {
            method: http::Method::POST,
            url,
            headers: Vec::new(),
            body,
            timeout: None,
        }
    }
}
