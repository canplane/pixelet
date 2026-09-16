// xpute_core/kit/hash.rs

//! The SHA-256 functions there are the host's WebCrypto and are not here
//! (script/pair.exempt.json).

use crate::kit::array::{Bytes, TypedArray};
use crate::kit::encoding::bytes_to_hex;

// ============ FNV-1a 64-bit, as one word ============

const FNV64_OFFSET: u64 = 0xcbf29ce484222325;
const FNV64_PRIME: u64 = 0x100000001b3;

/// FNV-1a over a string's UTF-16 code units, 64 bits wide, as the unsigned
/// word a 64-bit key lane holds — what a string keys a map by once the map
/// is lanes (ds/map.rs).
pub fn fnv1a64_str(s: &str) -> u64 {
    let mut h = FNV64_OFFSET;
    for unit in s.encode_utf16() {
        h ^= unit as u64;
        h = h.wrapping_mul(FNV64_PRIME);
    }
    h
}

/// The same word over a byte string, for an id built from numbers rather
/// than spelled out as text.
pub fn fnv1a64_bytes(bytes: &[u8]) -> u64 {
    let mut h = FNV64_OFFSET;
    for &b in bytes {
        h ^= b as u64;
        h = h.wrapping_mul(FNV64_PRIME);
    }
    h
}

// ============ FNV-1a 64-bit (8 bytes output) ============

pub struct FNVState {
    pub h1: u32,
    pub h2: u32,
    pub p: u32,
}

const FNV_OFFSET_32: u32 = 0x811c9dc5;
const FNV_PRIME_32: u32 = 0x01000193;

pub fn fnv1a_64_init() -> FNVState {
    FNVState {
        h1: FNV_OFFSET_32,
        h2: FNV_OFFSET_32,
        p: 0,
    }
}

pub fn fnv1a_64_u16le(st: &mut FNVState, x: u16) {
    let b0 = x as u8;
    let b1 = (x >> 8) as u8;
    fnv1a_64_byte(st, b0);
    fnv1a_64_byte(st, b1);
}

pub fn fnv1a_64_bytes(st: &mut FNVState, bytes: &Bytes) {
    for &b in bytes.as_slice() {
        fnv1a_64_byte(st, b);
    }
}

fn fnv1a_64_byte(st: &mut FNVState, b: u8) {
    let bx = b;

    let p = st.p;
    st.p = st.p.wrapping_add(1);
    if (p & 1) == 0 {
        st.h1 ^= bx as u32;
        st.h1 = st.h1.wrapping_mul(FNV_PRIME_32);
    } else {
        st.h2 ^= bx as u32;
        st.h2 = st.h2.wrapping_mul(FNV_PRIME_32);
    }
}

pub fn fnv1a_64_final(st: &FNVState) -> Bytes {
    let out: Bytes = TypedArray::new(8);
    out.as_mut_slice()[..4].copy_from_slice(&st.h1.to_le_bytes());
    out.as_mut_slice()[4..].copy_from_slice(&st.h2.to_le_bytes());
    out
}

pub fn fnv1a_64_hex(bytes: &Bytes) -> String {
    let mut st = fnv1a_64_init();
    fnv1a_64_bytes(&mut st, bytes);
    bytes_to_hex(&fnv1a_64_final(&st))
}

#[cfg(test)]
#[path = "hash.test.rs"]
mod test;
