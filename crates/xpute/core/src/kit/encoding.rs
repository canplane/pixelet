// xpute-core/kit/encoding.rs

//! Encoding utilities
//! - UTF-8
//! - Base64url (URL-safe, no padding)
//! - UUID v4
//! - Hex

#![allow(non_upper_case_globals)]

use crate::kit::array::{Bytes, TypedArray};
use crate::kit::rng;

// ============ UTF-8 ============

/// `TextEncoder`: a string to its UTF-8 bytes.
pub struct TextEncoder;

impl TextEncoder {
    pub fn encode(&self, s: &str) -> Bytes {
        TypedArray::from(s.as_bytes())
    }
}

/// `TextDecoder`: UTF-8 bytes to a string, an invalid sequence replaced by
/// U+FFFD as the default decoder does.
pub struct TextDecoder;

impl TextDecoder {
    pub fn decode(&self, b: &Bytes) -> String {
        String::from_utf8_lossy(b.as_slice()).into_owned()
    }
}

pub const te: TextEncoder = TextEncoder;
pub const td: TextDecoder = TextDecoder;

// ============ Base64url ============

pub type Base64URL = String; // 22 chars
type Base64 = String;

pub fn is_b64url(s: &str) -> bool {
    // base64url charset only (no '=')
    // NOTE: keep ASCII-only
    s.bytes().all(|c| c.is_ascii_digit() || c.is_ascii_uppercase() || c.is_ascii_lowercase() || c == b'-' || c == b'_')
}

pub fn ct_eq(a: &Bytes, b: &Bytes) -> bool {
    if a.byte_length() != b.byte_length() {
        return false;
    }
    let mut v = 0u8;
    for i in 0..a.byte_length() {
        v |= a[i] ^ b[i];
    }
    v == 0
}

const B64: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

/// `btoa`: bytes to standard base64 with padding.
fn btoa(bytes: &[u8]) -> Base64 {
    let mut out = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let n = chunk.len();
        let b = [chunk[0], if n > 1 { chunk[1] } else { 0 }, if n > 2 { chunk[2] } else { 0 }];
        let v = ((b[0] as u32) << 16) | ((b[1] as u32) << 8) | b[2] as u32;
        out.push(B64[(v >> 18) as usize & 63] as char);
        out.push(B64[(v >> 12) as usize & 63] as char);
        out.push(if n > 1 { B64[(v >> 6) as usize & 63] as char } else { '=' });
        out.push(if n > 2 { B64[v as usize & 63] as char } else { '=' });
    }
    out
}

/// `atob`: standard base64 to bytes; panics on a character outside the
/// alphabet, as `atob` throws.
fn atob(b64: &str) -> Vec<u8> {
    let mut out = Vec::with_capacity(b64.len() / 4 * 3);
    let mut acc: u32 = 0;
    let mut bits = 0;
    for c in b64.bytes() {
        if c == b'=' {
            break;
        }
        let v = B64.iter().position(|&x| x == c).unwrap_or_else(|| panic!("atob: the string to be decoded is not correctly encoded")) as u32;
        acc = (acc << 6) | v;
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            out.push((acc >> bits) as u8);
        }
    }
    out
}

/// Convert bytes to base64url
/// No padding, URL-safe
pub fn bytes_to_b64url(bytes: &Bytes) -> Base64URL {
    let b64: Base64 = btoa(bytes.as_slice());

    // base64 → base64url
    b64.replace('+', "-").replace('/', "_").replace('=', "")
}

/// Convert base64url to bytes
pub fn b64url_to_bytes(b64url: &str) -> Bytes {
    // base64url → base64
    let mut b64: Base64 = b64url.replace('-', "+").replace('_', "/");

    // restore padding
    while !b64.len().is_multiple_of(4) {
        b64.push('=');
    }

    TypedArray::from(&atob(&b64))
}

pub fn is_pid_b64url_16(pid: Option<&str>, pid_bin: &Bytes) -> bool {
    // strict: pid must be a base64url string encoding exactly 16 bytes, no padding.
    let Some(pid) = pid else { return false };
    if pid.len() != 22 {
        return false; // 16 bytes => 22 chars (unpadded base64url)
    }
    if !is_b64url(pid) {
        return false;
    }
    let b = b64url_to_bytes(pid);
    b.byte_length() == 16 && ct_eq(&b, pid_bin)
}

// ============ UUID v4 ============

pub type UUID = Bytes; // 16 bytes

/// Generate UUID v4 (random), from the entropy `kit/rng` has (the host's,
/// once installed; see `rng::seed64`).
pub fn generate_uuid() -> UUID {
    let mut bytes: UUID = TypedArray::new(16);
    let (a, b) = (rng::seed64().to_le_bytes(), rng::seed64().to_le_bytes());
    bytes.as_mut_slice()[..8].copy_from_slice(&a);
    bytes.as_mut_slice()[8..].copy_from_slice(&b);

    bytes[6] = (bytes[6] & 0x0f) | 0x40; // version 4
    bytes[8] = (bytes[8] & 0x3f) | 0x80; // variant 2

    bytes
}

/// UUID to base64url (22 chars)
pub fn uuid_to_b64url(uuid: &UUID) -> Base64URL {
    if uuid.length() != 16 {
        panic!("UUID: Invalid length");
    }
    bytes_to_b64url(uuid)
}

/// Base64url to UUID
pub fn b64url_to_uuid(b64url: &str) -> UUID {
    let bytes: UUID = b64url_to_bytes(b64url);
    if bytes.length() != 16 {
        panic!("UUID: Invalid format");
    }
    bytes
}

/// Compare UUIDs
pub fn uuid_equals(a: &UUID, b: &UUID) -> bool {
    if a.length() != 16 || b.length() != 16 {
        return false;
    }

    for i in 0..16 {
        if a[i] != b[i] {
            return false;
        }
    }

    true
}

// ============ Hex ============

/// Bytes to hex string
pub fn bytes_to_hex(bytes: &Bytes) -> String {
    bytes.as_slice().iter().map(|b| format!("{b:02x}")).collect()
}

/// Hex string to bytes
pub fn hex_to_bytes(hex: &str) -> Bytes {
    if !hex.len().is_multiple_of(2) {
        panic!("Hex: Invalid length");
    }

    let mut bytes = TypedArray::<u8>::new(hex.len() / 2);
    for i in (0..hex.len()).step_by(2) {
        bytes[i / 2] = u8::from_str_radix(&hex[i..i + 2], 16).unwrap_or_else(|_| panic!("Hex: Invalid digit"));
    }
    bytes
}
