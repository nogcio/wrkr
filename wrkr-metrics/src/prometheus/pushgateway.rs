use std::fmt::Write as _;
use std::io::{Read as _, Write as _};
use std::net::TcpStream;
use std::time::Duration;

use crate::Registry;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("invalid pushgateway base url: {0}")]
    InvalidBaseUrl(String),

    #[error("unsupported scheme in base url (only http:// is supported): {0}")]
    UnsupportedScheme(String),

    #[error("io error: {0}")]
    Io(#[from] std::io::Error),

    #[error("pushgateway returned non-success status {status_code}")]
    BadStatus {
        status_code: u16,
        response_prefix: String,
    },
}

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug, Clone)]
pub struct PushgatewayConfig {
    /// Base URL of Pushgateway, e.g. `http://127.0.0.1:9091`.
    pub base_url: String,
    /// Job name segment for Pushgateway, e.g. `wrkr`.
    pub job: String,
    /// Optional grouping labels: `/metrics/job/<job>/<k>/<v>/...`
    pub grouping_labels: Vec<(String, String)>,
    /// Request timeout.
    pub timeout: Duration,
}

impl PushgatewayConfig {
    #[must_use]
    pub fn new(base_url: impl Into<String>, job: impl Into<String>) -> Self {
        Self {
            base_url: base_url.into(),
            job: job.into(),
            grouping_labels: Vec::new(),
            timeout: Duration::from_secs(3),
        }
    }
}

/// Push a full `Registry` snapshot to a Prometheus Pushgateway.
///
/// This uses the Prometheus text exposition format as the payload and performs
/// a blocking HTTP/1.1 `PUT`.
///
/// Limitations:
/// - Only `http://` URLs are supported (no TLS).
pub fn push_registry(registry: &Registry, cfg: &PushgatewayConfig) -> Result<()> {
    let payload = super::registry_to_text(registry);
    push_text(&payload, cfg)
}

/// Push already-encoded Prometheus text exposition to Pushgateway.
pub fn push_text(text: &str, cfg: &PushgatewayConfig) -> Result<()> {
    let base = parse_http_base_url(&cfg.base_url)?;
    let path = join_path_prefix(
        &base.path_prefix,
        &build_path(&cfg.job, &cfg.grouping_labels),
    );

    let mut stream = TcpStream::connect((base.host.as_str(), base.port))?;
    stream.set_read_timeout(Some(cfg.timeout))?;
    stream.set_write_timeout(Some(cfg.timeout))?;

    let body = text.as_bytes();
    let mut req = String::new();
    req.push_str("PUT ");
    req.push_str(&path);
    req.push_str(" HTTP/1.1\r\n");
    req.push_str("Host: ");
    req.push_str(&base.host);
    if base.port != 80 {
        req.push(':');
        req.push_str(&base.port.to_string());
    }
    req.push_str("\r\n");
    req.push_str("Content-Type: text/plain; version=0.0.4; charset=utf-8\r\n");
    req.push_str("Content-Length: ");
    req.push_str(&body.len().to_string());
    req.push_str("\r\n");
    req.push_str("Connection: close\r\n\r\n");

    stream.write_all(req.as_bytes())?;
    stream.write_all(body)?;
    stream.flush()?;

    let mut resp = Vec::new();
    // We only need the headers + a small prefix.
    // Read until EOF or some reasonable cap.
    let mut buf = [0u8; 4096];
    while resp.len() < 64 * 1024 {
        let n = match stream.read(&mut buf) {
            Ok(0) => break,
            Ok(n) => n,
            Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => break,
            Err(e) if e.kind() == std::io::ErrorKind::TimedOut => break,
            Err(e) => return Err(Error::Io(e)),
        };
        resp.extend_from_slice(&buf[..n]);
    }

    let (status, prefix) = parse_status_and_prefix(&resp);
    match status {
        Some(code) if (200..=299).contains(&code) => Ok(()),
        Some(code) => Err(Error::BadStatus {
            status_code: code,
            response_prefix: prefix,
        }),
        None => Err(Error::BadStatus {
            status_code: 0,
            response_prefix: prefix,
        }),
    }
}

#[derive(Debug, Clone)]
struct HttpBase {
    host: String,
    port: u16,
    path_prefix: String,
}

fn parse_http_base_url(base_url: &str) -> Result<HttpBase> {
    let Some(rest) = base_url.strip_prefix("http://") else {
        if base_url.starts_with("https://") {
            return Err(Error::UnsupportedScheme(base_url.to_string()));
        }
        return Err(Error::InvalidBaseUrl(base_url.to_string()));
    };

    let (host_port, path_prefix) = match rest.split_once('/') {
        Some((hp, p)) => {
            let p = p.trim_matches('/');
            if p.is_empty() {
                (hp, String::new())
            } else {
                (hp, format!("/{p}"))
            }
        }
        None => (rest, String::new()),
    };

    if host_port.is_empty() {
        return Err(Error::InvalidBaseUrl(base_url.to_string()));
    }

    let (host, port) = match host_port.rsplit_once(':') {
        Some((h, p)) if !h.is_empty() && !p.is_empty() => {
            let port: u16 = p
                .parse()
                .map_err(|_| Error::InvalidBaseUrl(base_url.to_string()))?;
            (h.to_string(), port)
        }
        _ => (host_port.to_string(), 80),
    };

    Ok(HttpBase {
        host,
        port,
        path_prefix,
    })
}

fn join_path_prefix(prefix: &str, path: &str) -> String {
    if prefix.is_empty() {
        return path.to_string();
    }

    let prefix = prefix.trim_end_matches('/');
    let path = path.trim_start_matches('/');
    format!("{prefix}/{path}")
}

fn build_path(job: &str, grouping_labels: &[(String, String)]) -> String {
    // Pushgateway format: /metrics/job/<job>/<label_name>/<label_value>/...
    let mut out = String::new();
    out.push_str("/metrics/job/");
    out.push_str(&percent_encode_path_segment(job));

    for (k, v) in grouping_labels {
        out.push('/');
        out.push_str(&percent_encode_path_segment(k));
        out.push('/');
        out.push_str(&percent_encode_path_segment(v));
    }
    out
}

fn percent_encode_path_segment(s: &str) -> String {
    // RFC 3986 unreserved: ALPHA / DIGIT / "-" / "." / "_" / "~"
    fn is_unreserved(b: u8) -> bool {
        matches!(b, b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'.' | b'_' | b'~')
    }

    let bytes = s.as_bytes();
    let mut out = String::with_capacity(bytes.len());
    for &b in bytes {
        if is_unreserved(b) {
            out.push(b as char);
        } else {
            let _ = write!(out, "%{:02X}", b);
        }
    }
    out
}

fn parse_status_and_prefix(resp: &[u8]) -> (Option<u16>, String) {
    let s = String::from_utf8_lossy(resp);
    let prefix: String = s.chars().take(512).collect();
    let line = s.lines().next().unwrap_or("");
    // HTTP/1.1 202 Accepted
    let mut parts = line.split_whitespace();
    let _http = parts.next();
    let code = parts.next().and_then(|p| p.parse::<u16>().ok());
    (code, prefix)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn percent_encode_path_segment_encodes_slash_and_space() {
        assert_eq!(percent_encode_path_segment("a/b"), "a%2Fb");
        assert_eq!(percent_encode_path_segment("hello world"), "hello%20world");
    }

    #[test]
    fn build_path_includes_job_and_grouping() {
        let path = build_path("wrkr", &[("instance".to_string(), "local".to_string())]);
        assert_eq!(path, "/metrics/job/wrkr/instance/local");
    }

    #[test]
    fn parse_http_base_url_accepts_host_and_port() {
        let b = match parse_http_base_url("http://example.com:1234") {
            Ok(v) => v,
            Err(e) => panic!("unexpected error: {e}"),
        };
        assert_eq!(b.host, "example.com");
        assert_eq!(b.port, 1234);
        assert!(b.path_prefix.is_empty());
    }

    #[test]
    fn parse_http_base_url_keeps_path_prefix() {
        let b = match parse_http_base_url("http://example.com/prefix") {
            Ok(v) => v,
            Err(e) => panic!("unexpected error: {e}"),
        };
        assert_eq!(b.host, "example.com");
        assert_eq!(b.port, 80);
        assert_eq!(b.path_prefix, "/prefix");
        assert_eq!(
            join_path_prefix(&b.path_prefix, "/metrics/job/wrkr"),
            "/prefix/metrics/job/wrkr"
        );
    }

    #[test]
    fn parse_http_base_url_rejects_https() {
        let err = match parse_http_base_url("https://example.com") {
            Ok(_) => panic!("unexpected ok"),
            Err(e) => e,
        };
        match err {
            Error::UnsupportedScheme(_) => {}
            other => panic!("unexpected: {other:?}"),
        }
    }
}
