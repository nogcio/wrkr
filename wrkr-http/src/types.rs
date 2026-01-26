use std::time::Duration;

use bytes::Bytes;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HttpResponse {
    pub status: u16,
    pub body: Bytes,
    /// Response headers (lowercased header names). Multiple values are joined with ", ".
    pub headers: Vec<(String, String)>,
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
