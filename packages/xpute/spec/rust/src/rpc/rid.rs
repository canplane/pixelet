// @xpute/spec/rpc/rid.rs

/// RID: Request identity hash used for request matching/deduplication.
///
/// Format: 11-char base64url string
/// Hash: FNV-1a 64-bit (8 bytes)
pub type RID = String;

use crate::ipc::frame::Vector;
use crate::lib::kit::array::Bytes;

const FNV_OFFSET_32: u32 = 0x811c_9dc5;
const FNV_PRIME_32: u32 = 0x0100_0193;
const B64URL: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_";

/// Generate RID from (vec, tlv).
///
/// Contract:
/// - `tlv` is the TLV stream for the request identity payload.
/// - reply payload MUST NOT participate in RID derivation.
/// - It SHOULD be exactly the request-side TLV argument bytes used for RID derivation.
/// - `vec` is mixed as 2 bytes little-endian (no TLV overhead).
pub fn generate_rid(vec: Vector, tlv: Bytes) -> RID {
    let mut h1 = FNV_OFFSET_32;
    let mut h2 = FNV_OFFSET_32;
    let mut p = 0u32;

    for byte in vec.to_le_bytes() {
        fnv1a_64_byte(&mut h1, &mut h2, &mut p, byte);
    }

    for byte in tlv {
        fnv1a_64_byte(&mut h1, &mut h2, &mut p, byte);
    }

    let mut raw = [0u8; 8];
    raw[..4].copy_from_slice(&h1.to_le_bytes());
    raw[4..].copy_from_slice(&h2.to_le_bytes());
    base64url_no_pad(&raw)
}

fn fnv1a_64_byte(h1: &mut u32, h2: &mut u32, p: &mut u32, byte: u8) {
    if (*p & 1) == 0 {
        *h1 ^= u32::from(byte);
        *h1 = h1.wrapping_mul(FNV_PRIME_32);
    } else {
        *h2 ^= u32::from(byte);
        *h2 = h2.wrapping_mul(FNV_PRIME_32);
    }
    *p = p.wrapping_add(1);
}

fn base64url_no_pad(bytes: &[u8]) -> String {
    let mut out = String::with_capacity((bytes.len() * 4).div_ceil(3));
    let mut i = 0usize;

    while i + 3 <= bytes.len() {
        let chunk =
            (u32::from(bytes[i]) << 16) | (u32::from(bytes[i + 1]) << 8) | u32::from(bytes[i + 2]);
        out.push(B64URL[((chunk >> 18) & 0x3f) as usize] as char);
        out.push(B64URL[((chunk >> 12) & 0x3f) as usize] as char);
        out.push(B64URL[((chunk >> 6) & 0x3f) as usize] as char);
        out.push(B64URL[(chunk & 0x3f) as usize] as char);
        i += 3;
    }

    match bytes.len() - i {
        1 => {
            let chunk = u32::from(bytes[i]) << 16;
            out.push(B64URL[((chunk >> 18) & 0x3f) as usize] as char);
            out.push(B64URL[((chunk >> 12) & 0x3f) as usize] as char);
        }
        2 => {
            let chunk = (u32::from(bytes[i]) << 16) | (u32::from(bytes[i + 1]) << 8);
            out.push(B64URL[((chunk >> 18) & 0x3f) as usize] as char);
            out.push(B64URL[((chunk >> 12) & 0x3f) as usize] as char);
            out.push(B64URL[((chunk >> 6) & 0x3f) as usize] as char);
        }
        _ => {}
    }

    out
}

#[cfg(test)]
mod tests {
    use super::generate_rid;

    #[test]
    fn rid_is_stable_and_11_chars() {
        let rid = generate_rid(0x0011, vec![0x02, 0x7f, 0x00]);

        assert_eq!(rid, "7m06RiD-dlI");
        assert_eq!(rid.len(), 11);
    }
}
