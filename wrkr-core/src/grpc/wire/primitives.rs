use bytes::Buf as _;
use bytes::BufMut as _;

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub(super) enum WireType {
    Variant = 0,
    Bit64 = 1,
    Len = 2,
    Bit32 = 5,
}

impl TryFrom<u8> for WireType {
    type Error = String;

    fn try_from(value: u8) -> std::result::Result<Self, Self::Error> {
        Ok(match value {
            0 => Self::Variant,
            1 => Self::Bit64,
            2 => Self::Len,
            5 => Self::Bit32,
            other => return Err(format!("unsupported wire type {other}")),
        })
    }
}

pub(super) fn write_tag(field_number: u32, wire_type: WireType, out: &mut bytes::BytesMut) {
    let tag = (u64::from(field_number) << 3) | (wire_type as u64);
    write_variant(tag, out);
}

pub(super) fn write_len_delimited(bytes: &[u8], out: &mut bytes::BytesMut) {
    write_variant(bytes.len() as u64, out);
    out.put_slice(bytes);
}

pub(super) fn write_variant(mut v: u64, out: &mut bytes::BytesMut) {
    while v >= 0x80 {
        out.put_u8(((v as u8) & 0x7F) | 0x80);
        v >>= 7;
    }
    out.put_u8(v as u8);
}

pub(super) fn read_variant(src: &mut bytes::Bytes) -> std::result::Result<u64, String> {
    let mut shift = 0;
    let mut out: u64 = 0;

    for _ in 0..10 {
        if !src.has_remaining() {
            return Err("unexpected EOF reading variant".to_string());
        }
        let b = src.get_u8();
        out |= u64::from(b & 0x7F) << shift;
        if (b & 0x80) == 0 {
            return Ok(out);
        }
        shift += 7;
    }

    Err("variant too long".to_string())
}

pub(super) fn read_len_delimited(
    src: &mut bytes::Bytes,
) -> std::result::Result<bytes::Bytes, String> {
    let len = read_variant(src)? as usize;
    if src.remaining() < len {
        return Err("unexpected EOF reading len-delimited".to_string());
    }
    Ok(src.copy_to_bytes(len))
}

pub(super) fn skip_value(
    wire_type: WireType,
    src: &mut bytes::Bytes,
) -> std::result::Result<(), String> {
    match wire_type {
        WireType::Variant => {
            let _ = read_variant(src)?;
        }
        WireType::Bit64 => {
            if src.remaining() < 8 {
                return Err("unexpected EOF skipping 64-bit".to_string());
            }
            src.advance(8);
        }
        WireType::Len => {
            let len = read_variant(src)? as usize;
            if src.remaining() < len {
                return Err("unexpected EOF skipping len".to_string());
            }
            src.advance(len);
        }
        WireType::Bit32 => {
            if src.remaining() < 4 {
                return Err("unexpected EOF skipping 32-bit".to_string());
            }
            src.advance(4);
        }
    }

    Ok(())
}

pub(super) fn encode_zigzag64(v: i64) -> u64 {
    ((v << 1) ^ (v >> 63)) as u64
}

pub(super) fn decode_zigzag64(v: u64) -> i64 {
    ((v >> 1) as i64) ^ (-((v & 1) as i64))
}
