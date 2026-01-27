use bytes::Bytes;
use http_body_util::{BodyExt as _, Full};
use hyper::Request;
use hyper::body::Incoming;
use hyper_rustls::{HttpsConnector, HttpsConnectorBuilder};
use hyper_util::client::legacy::Client;
use hyper_util::client::legacy::connect::HttpConnector;
use hyper_util::rt::TokioExecutor;
use std::collections::BTreeMap;
use std::time::Duration;

use super::estimate::{estimate_http_request_bytes_parts, estimate_http1_response_head_bytes};
use super::util::{has_header, host_header_value};
use super::{Error, HttpRequest, HttpResponse, Result};

#[derive(Debug, Clone)]
pub struct HttpClient {
    inner: Client<HttpsConnector<HttpConnector>, Full<Bytes>>,
}

impl Default for HttpClient {
    fn default() -> Self {
        // The OS-level TCP connect timeout can be very long (tens of seconds), which can cause
        // short runs to appear “hung” when the target host is unreachable.
        //
        // We apply a sane default so failed connects surface promptly.
        Self::new(Some(Duration::from_secs(3)))
    }
}

impl HttpClient {
    #[must_use]
    pub fn new(connect_timeout: Option<Duration>) -> Self {
        let mut http_connector = HttpConnector::new();
        http_connector.enforce_http(false);
        http_connector.set_connect_timeout(connect_timeout);

        let https_connector = HttpsConnectorBuilder::new()
            .with_webpki_roots()
            .https_or_http()
            .enable_http1()
            .wrap_connector(http_connector);

        let inner = Client::builder(TokioExecutor::new()).build(https_connector);

        Self { inner }
    }

    pub async fn request(&self, req: HttpRequest) -> Result<HttpResponse> {
        let timeout = req.timeout;
        let parsed = url::Url::parse(&req.url).map_err(|_| Error::InvalidUrl(req.url.clone()))?;
        if parsed.scheme() != "http" && parsed.scheme() != "https" {
            return Err(Error::UnsupportedScheme(req.url));
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

        // Normalize headers to lowercase keys for scripting ergonomics.
        // If there are multiple values for a header, join them with ", ".
        let mut merged: BTreeMap<String, String> = BTreeMap::new();
        for (name, value) in parts.headers.iter() {
            let key = name.as_str().to_ascii_lowercase();
            let v = String::from_utf8_lossy(value.as_bytes()).to_string();
            merged
                .entry(key)
                .and_modify(|cur| {
                    if !cur.is_empty() {
                        cur.push_str(", ");
                    }
                    cur.push_str(&v);
                })
                .or_insert(v);
        }
        let headers: Vec<(String, String)> = merged.into_iter().collect();

        let head_bytes =
            estimate_http1_response_head_bytes(parts.version, parts.status, &parts.headers);
        let body = body.collect().await?.to_bytes();
        let bytes_received = head_bytes.saturating_add(body.len() as u64);

        Ok(HttpResponse {
            status,
            body,
            headers,
            bytes_sent,
            bytes_received,
        })
    }

    pub async fn get(&self, url: &str) -> Result<HttpResponse> {
        self.request(HttpRequest::get(url)).await
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use super::*;
    use std::time::Instant;

    #[tokio::test]
    async fn unreachable_host_fails_fast_with_connect_timeout() {
        // Use a small timeout to keep the test fast and deterministic.
        let client = HttpClient::new(Some(Duration::from_millis(200)));
        let req = HttpRequest::get("http://192.0.2.1:81/");

        let started = Instant::now();
        let _err = client.request(req).await.unwrap_err();
        let elapsed = started.elapsed();

        // Assert we didn't block for an OS-level TCP connect timeout.
        assert!(
            elapsed < Duration::from_secs(2),
            "expected fast failure, elapsed={elapsed:?}"
        );
    }
}
