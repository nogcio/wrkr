#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, strum::Display, strum::EnumString)]
#[strum(serialize_all = "snake_case")]
pub enum GrpcTransportErrorKind {
    InvalidEndpoint,
    Connect,
    MetadataKey,
    MetadataValue,
    InvalidMethodPath,
    Encode,
}
