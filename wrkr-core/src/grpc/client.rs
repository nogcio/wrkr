use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Instant;

use tonic::metadata::{MetadataKey, MetadataValue};
use tonic::transport::{Certificate, Channel, ClientTlsConfig, Endpoint, Identity};

use crate::proto::GrpcMethod;

use super::codec_bytes::BytesCodec;
use super::metadata::metadata_to_pairs;
use super::wire::{decode_value_for_method, encode_value_for_method};
use super::{ConnectOptions, Error, InvokeOptions, Result, UnaryResult};

#[derive(Debug, Clone)]
pub struct GrpcClient {
    channels: Arc<[Channel]>,
    rr: Arc<AtomicUsize>,
}

impl GrpcClient {
    async fn unary_inner(
        &self,
        method: &GrpcMethod,
        req_bytes: bytes::Bytes,
        opts: InvokeOptions,
    ) -> Result<UnaryResult> {
        let started = Instant::now();

        let path = method.path().clone();

        let bytes_sent = req_bytes.len() as u64;
        let mut request = tonic::Request::new(req_bytes);

        if let Some(timeout) = opts.timeout {
            request.set_timeout(timeout);
        }

        for (k, v) in opts.metadata {
            let key =
                MetadataKey::from_bytes(k.as_bytes()).map_err(|_| Error::MetadataKey(k.clone()))?;
            let value = MetadataValue::try_from(v.clone())
                .map_err(|_| Error::MetadataValue { key: k, value: v })?;
            request.metadata_mut().insert(key, value);
        }

        let i = self.rr.fetch_add(1, Ordering::Relaxed);
        // Invariant: connect_pooled ensures at least 1 channel.
        let channel = self.channels[i % self.channels.len()].clone();
        let mut grpc = tonic::client::Grpc::new(channel);
        let codec = BytesCodec;

        let res = async {
            grpc.ready()
                .await
                .map_err(|e| tonic::Status::unknown(format!("Service was not ready: {e}")))?;
            grpc.unary(request, path, codec).await
        }
        .await;

        let elapsed = started.elapsed();

        match res {
            Ok(res) => {
                let headers = metadata_to_pairs(res.metadata());
                let decoded = res.into_inner();
                let bytes_received = decoded.bytes.len() as u64;

                let response = decode_value_for_method(method, decoded.bytes.clone())
                    .map_err(Error::Decode)?;

                Ok(UnaryResult {
                    ok: true,
                    status: Some(0),
                    message: None,
                    error: None,
                    transport_error_kind: None,
                    response,
                    headers,
                    trailers: Vec::new(),
                    elapsed,
                    bytes_sent,
                    bytes_received,
                })
            }
            Err(status) => {
                // Non-OK gRPC status is a normal protocol outcome.
                let code = status.code() as u16;
                let trailers = metadata_to_pairs(status.metadata());

                Ok(UnaryResult {
                    ok: false,
                    status: Some(code),
                    message: Some(status.message().to_string()),
                    error: Some(status.to_string()),
                    transport_error_kind: None,
                    response: wrkr_value::Value::Null,
                    headers: Vec::new(),
                    trailers,
                    elapsed,
                    bytes_sent,
                    bytes_received: 0,
                })
            }
        }
    }

    pub async fn connect(target: &str, opts: ConnectOptions) -> Result<Self> {
        Self::connect_pooled(target, opts, 1).await
    }

    pub async fn connect_pooled(
        target: &str,
        opts: ConnectOptions,
        pool_size: usize,
    ) -> Result<Self> {
        let pool_size = pool_size.max(1);

        let uri = if target.contains("://") {
            target.to_string()
        } else if opts.tls.is_some() {
            format!("https://{target}")
        } else {
            format!("http://{target}")
        };

        let mut endpoint = Endpoint::from_shared(uri)?;

        // Throughput-sensitive defaults for local perf runs.
        // A larger buffer reduces time spent waiting for tower Buffer capacity
        // under high VU counts.
        endpoint = endpoint
            .tcp_nodelay(true)
            .buffer_size(4096)
            .http2_adaptive_window(false);

        if let Some(timeout) = opts.timeout {
            endpoint = endpoint.connect_timeout(timeout);
        }

        if let Some(tls) = opts.tls {
            let mut tls_cfg = ClientTlsConfig::new();

            if let Some(domain) = tls.domain_name {
                tls_cfg = tls_cfg.domain_name(domain);
            }

            let _ = tls.insecure_skip_verify;

            if let Some(ca_pem) = tls.ca_pem {
                tls_cfg = tls_cfg.ca_certificate(Certificate::from_pem(ca_pem));
            }

            if let (Some(cert), Some(key)) = (tls.identity_pem, tls.identity_key_pem) {
                tls_cfg = tls_cfg.identity(Identity::from_pem(cert, key));
            }

            endpoint = endpoint.tls_config(tls_cfg)?;
        }

        let mut channels: Vec<Channel> = Vec::with_capacity(pool_size);
        for _ in 0..pool_size {
            let channel = endpoint.clone().connect().await.map_err(Error::Connect)?;
            channels.push(channel);
        }

        Ok(Self {
            channels: Arc::from(channels.into_boxed_slice()),
            rr: Arc::new(AtomicUsize::new(0)),
        })
    }

    pub async fn unary(
        &self,
        method: &GrpcMethod,
        req: wrkr_value::Value,
        opts: InvokeOptions,
    ) -> Result<UnaryResult> {
        let bytes = encode_value_for_method(method, &req).map_err(Error::Encode)?;
        self.unary_inner(method, bytes, opts).await
    }

    pub async fn unary_bytes(
        &self,
        method: &GrpcMethod,
        req_bytes: bytes::Bytes,
        opts: InvokeOptions,
    ) -> Result<UnaryResult> {
        self.unary_inner(method, req_bytes, opts).await
    }
}
