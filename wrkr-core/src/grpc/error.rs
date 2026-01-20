use super::GrpcTransportErrorKind;

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("invalid endpoint: {0}")]
    InvalidEndpoint(#[from] tonic::transport::Error),

    #[error("failed to connect: {0}")]
    Connect(#[source] tonic::transport::Error),

    #[error("invalid metadata key: {0}")]
    MetadataKey(String),

    #[error("invalid metadata value for '{key}': {value}")]
    MetadataValue { key: String, value: String },

    #[error("invalid gRPC method path")]
    InvalidMethodPath,

    #[error("failed to encode request: {0}")]
    Encode(String),
}

impl Error {
    #[must_use]
    pub fn transport_error_kind(&self) -> GrpcTransportErrorKind {
        match self {
            Self::InvalidEndpoint(_) => GrpcTransportErrorKind::InvalidEndpoint,
            Self::Connect(_) => GrpcTransportErrorKind::Connect,
            Self::MetadataKey(_) => GrpcTransportErrorKind::MetadataKey,
            Self::MetadataValue { .. } => GrpcTransportErrorKind::MetadataValue,
            Self::InvalidMethodPath => GrpcTransportErrorKind::InvalidMethodPath,
            Self::Encode(_) => GrpcTransportErrorKind::Encode,
        }
    }
}
