use bytes::Bytes;
use http_body_util::{BodyExt as _, Full};
use hyper::Request;
use hyper::body::Incoming;
use hyper_util::client::legacy::Client;
use hyper_util::client::legacy::connect::HttpConnector;
use hyper_util::rt::TokioExecutor;

use super::estimate::{estimate_http_request_bytes_parts, estimate_http1_response_head_bytes};
use super::util::{has_header, host_header_value};
use super::{Error, HttpRequest, HttpResponse, Result};

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
