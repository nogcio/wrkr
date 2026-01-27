use std::borrow::Cow;

use super::util::{has_header, host_header_value};
use super::{Error, HttpRequest, Result};

/// Estimate bytes sent for an HTTP request.
///
/// This is a best-effort estimate of HTTP/1.1 framing: request line + headers + CRLF + body.
/// It intentionally makes Host/Content-Length explicit (if they are missing) for determinism.
pub fn estimate_http_request_bytes(req: &HttpRequest) -> Result<u64> {
    estimate_http_request_bytes_parts(&req.method, &req.url, &req.headers, req.body.len() as u64)
}

pub(super) fn estimate_http_request_bytes_parts(
    method: &http::Method,
    url: &str,
    headers: &[(String, String)],
    body_len: u64,
) -> Result<u64> {
    let parsed = url::Url::parse(url).map_err(|_| Error::InvalidUrl(url.to_string()))?;
    if parsed.scheme() != "http" && parsed.scheme() != "https" {
        return Err(Error::UnsupportedScheme(url.to_string()));
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

pub(super) fn estimate_http1_response_head_bytes(
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
