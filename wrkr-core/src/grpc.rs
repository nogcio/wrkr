mod client;
mod codec_bytes;
mod error;
mod kind;
mod metadata;
mod types;
mod wire;

pub use client::GrpcClient;
pub use error::{Error, Result};
pub use kind::GrpcTransportErrorKind;
pub use types::{ConnectOptions, InvokeOptions, TlsConfig, UnaryResult};

pub fn encode_unary_request(
    method: &crate::GrpcMethod,
    value: &wrkr_value::Value,
) -> Result<bytes::Bytes> {
    wire::encode_value_for_method(method, value).map_err(Error::Encode)
}
