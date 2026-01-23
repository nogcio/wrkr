use bytes::Buf as _;
use prost::Message as _;

#[derive(Debug)]
pub(super) struct DecodedDynamicMessage {
    pub(super) msg: prost_reflect::DynamicMessage,
}

#[derive(Clone)]
pub(super) struct DynamicMessageCodec {
    response_desc: prost_reflect::MessageDescriptor,
}

impl DynamicMessageCodec {
    pub(super) fn new(response_desc: prost_reflect::MessageDescriptor) -> Self {
        Self { response_desc }
    }
}

impl tonic::codec::Codec for DynamicMessageCodec {
    type Encode = prost_reflect::DynamicMessage;
    type Decode = DecodedDynamicMessage;
    type Encoder = DynamicMessageEncoder;
    type Decoder = DynamicMessageDecoder;

    fn encoder(&mut self) -> Self::Encoder {
        DynamicMessageEncoder
    }

    fn decoder(&mut self) -> Self::Decoder {
        DynamicMessageDecoder {
            desc: self.response_desc.clone(),
        }
    }
}

#[derive(Clone)]
pub(super) struct DynamicMessageEncoder;

impl tonic::codec::Encoder for DynamicMessageEncoder {
    type Item = prost_reflect::DynamicMessage;
    type Error = tonic::Status;

    fn encode(
        &mut self,
        item: Self::Item,
        dst: &mut tonic::codec::EncodeBuf<'_>,
    ) -> std::result::Result<(), Self::Error> {
        item.encode(dst)
            .map_err(|e| tonic::Status::internal(e.to_string()))?;
        Ok(())
    }
}

#[derive(Clone)]
pub(super) struct DynamicMessageDecoder {
    desc: prost_reflect::MessageDescriptor,
}

impl tonic::codec::Decoder for DynamicMessageDecoder {
    type Item = DecodedDynamicMessage;
    type Error = tonic::Status;

    fn decode(
        &mut self,
        src: &mut tonic::codec::DecodeBuf<'_>,
    ) -> std::result::Result<Option<Self::Item>, Self::Error> {
        if !src.has_remaining() {
            return Ok(None);
        }

        let msg = prost_reflect::DynamicMessage::decode(self.desc.clone(), &mut *src)
            .map_err(|e| tonic::Status::internal(e.to_string()))?;

        Ok(Some(DecodedDynamicMessage { msg }))
    }
}
