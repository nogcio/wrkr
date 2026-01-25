use bytes::{Buf as _, BufMut as _};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum WireType {
    Varint,
    SixtyFourBit,
    Len,
    ThirtyTwoBit,
}

impl TryFrom<u8> for WireType {
    type Error = String;

    fn try_from(v: u8) -> Result<Self, Self::Error> {
        match v {
            0 => Ok(Self::Varint),
            1 => Ok(Self::SixtyFourBit),
            2 => Ok(Self::Len),
            5 => Ok(Self::ThirtyTwoBit),
            other => Err(format!("unsupported protobuf wire type {other}")),
        }
    }
}

pub(super) fn write_tag(field_number: u32, wire_type: WireType, out: &mut bytes::BytesMut) {
    let wt: u64 = match wire_type {
        WireType::Varint => 0,
        WireType::SixtyFourBit => 1,
        WireType::Len => 2,
        WireType::ThirtyTwoBit => 5,
    };
    let tag = ((field_number as u64) << 3) | wt;
    write_variant(tag, out);
}

pub(super) fn write_variant(mut v: u64, out: &mut bytes::BytesMut) {
    while v >= 0x80 {
        out.put_u8((v as u8) | 0x80);
        v >>= 7;
    }
    out.put_u8(v as u8);
}

pub(super) fn read_variant(src: &mut bytes::Bytes) -> Result<u64, String> {
    let mut shift: u32 = 0;
    let mut out: u64 = 0;

    while src.has_remaining() {
        let b = src.get_u8();
        out |= ((b & 0x7f) as u64) << shift;

        if (b & 0x80) == 0 {
            return Ok(out);
        }

        shift += 7;
        if shift >= 64 {
            break;
        }
    }

    Err("invalid varint".to_string())
}

pub(super) fn write_len_delimited(bytes: bytes::Bytes, out: &mut bytes::BytesMut) {
    write_variant(bytes.len() as u64, out);
    out.put_slice(&bytes);
}

pub(super) fn read_len_delimited(src: &mut bytes::Bytes) -> Result<bytes::Bytes, String> {
    let len = read_variant(src)? as usize;
    if src.remaining() < len {
        return Err("invalid length-delimited field".to_string());
    }
    Ok(src.split_to(len))
}

pub(super) fn skip_value(wire_type: WireType, src: &mut bytes::Bytes) -> Result<(), String> {
    match wire_type {
        WireType::Varint => {
            let _ = read_variant(src)?;
            Ok(())
        }
        WireType::SixtyFourBit => {
            if src.remaining() < 8 {
                return Err("invalid fixed64".to_string());
            }
            src.advance(8);
            Ok(())
        }
        WireType::Len => {
            let _ = read_len_delimited(src)?;
            Ok(())
        }
        WireType::ThirtyTwoBit => {
            if src.remaining() < 4 {
                return Err("invalid fixed32".to_string());
            }
            src.advance(4);
            Ok(())
        }
    }
}

pub(super) fn encode_zigzag64(v: i64) -> u64 {
    ((v << 1) ^ (v >> 63)) as u64
}

pub(super) fn decode_zigzag64(v: u64) -> i64 {
    ((v >> 1) as i64) ^ (-((v & 1) as i64))
}
