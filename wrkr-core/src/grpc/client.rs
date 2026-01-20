use std::time::Instant;

use prost::Message as _;
use tonic::codegen::http::uri::PathAndQuery;
use tonic::metadata::{MetadataKey, MetadataValue};
use tonic::transport::{Certificate, Channel, ClientTlsConfig, Endpoint, Identity};

use crate::proto::GrpcMethod;

use super::codec::DynamicMessageCodec;
use super::convert::{dynamic_message_to_value, value_to_dynamic_message};
use super::metadata::metadata_to_pairs;
use super::{ConnectOptions, Error, InvokeOptions, Result, UnaryResult};

#[derive(Debug, Clone)]
pub struct GrpcClient {
    channel: Channel,
}

impl GrpcClient {
    pub async fn connect(target: &str, opts: ConnectOptions) -> Result<Self> {
        let uri = if target.contains("://") {
            target.to_string()
        } else if opts.tls.is_some() {
            format!("https://{target}")
        } else {
            format!("http://{target}")
        };

        let mut endpoint = Endpoint::from_shared(uri)?;

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

        let channel = endpoint.connect().await.map_err(Error::Connect)?;
        Ok(Self { channel })
    }

    pub async fn unary(
        &self,
        method: &GrpcMethod,
        req: wrkr_value::Value,
        opts: InvokeOptions,
    ) -> Result<UnaryResult> {
        let started = Instant::now();

        let method = method.descriptor();

        let service = method.parent_service().full_name();
        let name = method.name();
        let path =
            PathAndQuery::from_maybe_shared(bytes::Bytes::from(format!("/{service}/{name}")))
                .map_err(|_| Error::InvalidMethodPath)?;

        let req = value_to_dynamic_message(method.input(), req).map_err(Error::Encode)?;

        let bytes_sent = req.encoded_len() as u64;

        let mut request = tonic::Request::new(req);

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

        let mut grpc = tonic::client::Grpc::new(self.channel.clone());
        let codec = DynamicMessageCodec::new(method.output());

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
                let msg: prost_reflect::DynamicMessage = res.into_inner();
                let bytes_received = msg.encoded_len() as u64;

                let response = dynamic_message_to_value(&msg);
                Ok(UnaryResult {
                    ok: true,
                    status: Some(0),
                    message: None,
                    error: None,
                    transport_error_kind: None,
                    response: Some(response),
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
                    response: None,
                    headers: Vec::new(),
                    trailers,
                    elapsed,
                    bytes_sent,
                    bytes_received: 0,
                })
            }
        }
    }
}
