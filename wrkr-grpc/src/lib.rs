#![forbid(unsafe_code)]

mod client;
mod codec_bytes;
mod error;
mod kind;
mod metadata;
mod proto;
pub mod shared;
mod types;
mod wire;

pub use client::GrpcClient;
pub use error::{Error, Result};
pub use kind::GrpcTransportErrorKind;
pub use proto::{Error as ProtoError, GrpcMethod, ProtoSchema};
pub use shared::SharedGrpcRegistry;
pub use types::{ConnectOptions, InvokeOptions, TlsConfig, UnaryResult};

/// Encode a unary request body for `method` using the protobuf schema metadata and `wrkr_value`
/// input.
///
/// This produces the protobuf wire bytes that should be sent as the gRPC request message.
pub fn encode_unary_request(
    method: &GrpcMethod,
    value: &wrkr_value::Value,
) -> Result<bytes::Bytes> {
    wire::encode_value_for_method(method, value).map_err(Error::Encode)
}
