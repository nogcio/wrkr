use bytes::Buf as _;
use bytes::BufMut as _;

#[derive(Debug, Clone)]
pub(crate) struct DecodedBytes {
    pub(crate) bytes: bytes::Bytes,
}

#[derive(Clone)]
pub(crate) struct BytesCodec;

impl tonic::codec::Codec for BytesCodec {
    type Encode = bytes::Bytes;
    type Decode = DecodedBytes;
    type Encoder = BytesEncoder;
    type Decoder = BytesDecoder;

    fn encoder(&mut self) -> Self::Encoder {
        BytesEncoder
    }

    fn decoder(&mut self) -> Self::Decoder {
        BytesDecoder
    }
}

#[derive(Clone)]
pub(crate) struct BytesEncoder;

impl tonic::codec::Encoder for BytesEncoder {
    type Item = bytes::Bytes;
    type Error = tonic::Status;

    fn encode(
        &mut self,
        item: Self::Item,
        dst: &mut tonic::codec::EncodeBuf<'_>,
    ) -> std::result::Result<(), Self::Error> {
        dst.put_slice(item.as_ref());
        Ok(())
    }
}

#[derive(Clone)]
pub(crate) struct BytesDecoder;

impl tonic::codec::Decoder for BytesDecoder {
    type Item = DecodedBytes;
    type Error = tonic::Status;

    fn decode(
        &mut self,
        src: &mut tonic::codec::DecodeBuf<'_>,
    ) -> std::result::Result<Option<Self::Item>, Self::Error> {
        if !src.has_remaining() {
            return Ok(None);
        }

        let bytes = src.copy_to_bytes(src.remaining());
        Ok(Some(DecodedBytes { bytes }))
    }
}
