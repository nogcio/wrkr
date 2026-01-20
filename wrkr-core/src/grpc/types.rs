use std::time::Duration;

use super::GrpcTransportErrorKind;

#[derive(Debug, Clone, Default)]
pub struct TlsConfig {
    pub ca_pem: Option<Vec<u8>>,
    pub identity_pem: Option<Vec<u8>>,
    pub identity_key_pem: Option<Vec<u8>>,
    pub domain_name: Option<String>,
    pub insecure_skip_verify: bool,
}

#[derive(Debug, Clone, Default)]
pub struct ConnectOptions {
    pub timeout: Option<Duration>,
    pub tls: Option<TlsConfig>,
}

#[derive(Debug, Clone, Default)]
pub struct InvokeOptions {
    pub timeout: Option<Duration>,
    pub metadata: Vec<(String, String)>,
}

#[derive(Debug, Clone)]
pub struct UnaryResult {
    pub ok: bool,
    pub status: Option<u16>,
    pub message: Option<String>,
    pub error: Option<String>,
    pub transport_error_kind: Option<GrpcTransportErrorKind>,

    pub response: Option<wrkr_value::Value>,
    pub headers: Vec<(String, String)>,
    pub trailers: Vec<(String, String)>,

    pub elapsed: Duration,
    pub bytes_sent: u64,
    pub bytes_received: u64,
}
