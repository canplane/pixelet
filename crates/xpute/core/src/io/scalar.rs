// xpute_core/io/scalar.rs

//! Binary Scalar I/O
//!
//! - All operations are little-endian by specification.
//! - No validation. Caller owns correctness.
//!
//! RULE
//! ----
//! This is IO only: raw byte stream read/write.
//! Signedness is a *meaning* decision; we expose both geti/getu for native widths,
//! but we do not "fix" or "reinterpret" beyond what a byte read already does.
//!
//! Composite widths (96/128) are limb-IO helpers.
//!
//! The TypeScript stream carries a `DataView` beside its bytes; the reads
//! here go through the bytes themselves, so a stream is the bytes and an
//! offset.

use crate::kit::array::{Bytes, TypedArray};

// ----------------------------------------------------------------------------
// Stream IO (little-endian)
// - No validation: caller owns bounds checks.
// ----------------------------------------------------------------------------

pub struct Stream {
    pub buf: Bytes,
    pub off: i32,
}

pub fn stream(buf: Bytes, off: i32) -> Stream {
    Stream { buf, off }
}

// ----------------------------------------------------------------------------
// memcpy (raw memory copy)
// - No validation: caller owns correctness (a panic is fine)
// ----------------------------------------------------------------------------

pub fn memcpy(dst: &Bytes, src: &Bytes, nbyte: u32) {
    // force exact-length source view; dst range is enforced by dst view length via .set()
    let s = TypedArray::<u8>::over(src.buffer().clone(), src.byte_offset(), nbyte as usize);
    dst.set_at(&s, 0); // panics if dst.byte_length < n
}

pub fn write(s: &mut Stream, src: &Bytes, nbyte: u32) {
    let beg = s.off;
    let end = beg + nbyte as i32;
    // exact-length view: panics if src is shorter than n
    let src_n = TypedArray::<u8>::over(src.buffer().clone(), src.byte_offset(), nbyte as usize);
    s.buf.set_at(&src_n, beg as usize);
    s.off = end;
}
pub fn read(s: &mut Stream, dst: &Bytes, nbyte: u32) {
    let beg = s.off;
    let end = beg + nbyte as i32;
    // exact-length view: panics if stream doesn't have n bytes
    let src_n = TypedArray::<u8>::over(s.buf.buffer().clone(), s.buf.byte_offset() + beg as usize, nbyte as usize);
    dst.set_at(&src_n, 0);
    s.off = end;
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(i32)]
pub enum Whence {
    SET = 0, // SEEK_SET
    CUR = 1, // SEEK_CUR
    END = 2, // SEEK_END
}

pub fn seek(s: &mut Stream, off: i32, whence: Whence) -> i32 {
    if whence == Whence::CUR {
        s.off += off;
    } else if whence == Whence::END {
        s.off = s.buf.byte_length() as i32 + off;
    } else {
        s.off = off;
    }
    s.off
}

pub fn tell(s: &Stream) -> i32 {
    s.off
}
pub fn len(s: &Stream) -> i32 {
    s.buf.byte_length() as i32
}
pub fn rem(s: &Stream) -> i32 {
    s.buf.byte_length() as i32 - s.off
}

// ----------------------------------------------------------------------------
// scalar ops (advance stream)
// ----------------------------------------------------------------------------

fn put<const N: usize>(s: &mut Stream, bytes: [u8; N]) {
    let at = s.off as usize;
    s.off += N as i32;
    s.buf.as_mut_slice()[at..at + N].copy_from_slice(&bytes);
}

fn get<const N: usize>(s: &mut Stream) -> [u8; N] {
    let at = s.off as usize;
    s.off += N as i32;
    s.buf.as_slice()[at..at + N].try_into().unwrap()
}

pub fn putu8(s: &mut Stream, x: u8) {
    put(s, x.to_le_bytes())
}
pub fn puti8(s: &mut Stream, x: i8) {
    put(s, x.to_le_bytes())
}
pub fn getu8(s: &mut Stream) -> u8 {
    u8::from_le_bytes(get(s))
}
pub fn geti8(s: &mut Stream) -> i8 {
    i8::from_le_bytes(get(s))
}

pub fn putu16(s: &mut Stream, x: u16) {
    put(s, x.to_le_bytes())
}
pub fn puti16(s: &mut Stream, x: i16) {
    put(s, x.to_le_bytes())
}
pub fn getu16(s: &mut Stream) -> u16 {
    u16::from_le_bytes(get(s))
}
pub fn geti16(s: &mut Stream) -> i16 {
    i16::from_le_bytes(get(s))
}

pub fn putu32(s: &mut Stream, x: u32) {
    put(s, x.to_le_bytes())
}
pub fn puti32(s: &mut Stream, x: i32) {
    put(s, x.to_le_bytes())
}
pub fn getu32(s: &mut Stream) -> u32 {
    u32::from_le_bytes(get(s))
}
pub fn geti32(s: &mut Stream) -> i32 {
    i32::from_le_bytes(get(s))
}

pub fn putu64(s: &mut Stream, x: u64) {
    put(s, x.to_le_bytes())
}
pub fn puti64(s: &mut Stream, x: i64) {
    put(s, x.to_le_bytes())
}
pub fn getu64(s: &mut Stream) -> u64 {
    u64::from_le_bytes(get(s))
}
pub fn geti64(s: &mut Stream) -> i64 {
    i64::from_le_bytes(get(s))
}

pub fn putf32(s: &mut Stream, x: f32) {
    put(s, x.to_le_bytes())
}
pub fn getf32(s: &mut Stream) -> f32 {
    f32::from_le_bytes(get(s))
}

pub fn putf64(s: &mut Stream, x: f64) {
    put(s, x.to_le_bytes())
}
pub fn getf64(s: &mut Stream) -> f64 {
    f64::from_le_bytes(get(s))
}

// ----------------------------------------------------------------------------
// 96/128 as limb stream IO (little-endian)
// ----------------------------------------------------------------------------

pub const U96_SZ: u32 = 12;
pub const U128_SZ: u32 = 16;

pub fn putu96(s: &mut Stream, lo: u32, mid: u32, hi: u32) {
    putu32(s, lo);
    putu32(s, mid);
    putu32(s, hi);
}

pub fn getu96(s: &mut Stream) -> [u32; 3] {
    let lo = getu32(s);
    let mid = getu32(s);
    let hi = getu32(s);
    [lo, mid, hi]
}

pub fn putu128(s: &mut Stream, lo: u64, hi: u64) {
    putu64(s, lo);
    putu64(s, hi);
}

pub fn getu128(s: &mut Stream) -> [u64; 2] {
    let lo = getu64(s);
    let hi = getu64(s);
    [lo, hi]
}

// ----------------------------------------------------------------------------
// 96/128 bigint <-> limb (keep)
// ----------------------------------------------------------------------------

pub fn pack96(lo: u32, mid: u32, hi: u32) -> u128 {
    ((hi as u128) << 64) | ((mid as u128) << 32) | lo as u128
}

pub fn unpack96(x: u128) -> [u32; 3] {
    [x as u32, (x >> 32) as u32, (x >> 64) as u32]
}

pub fn pack128(lo: u64, hi: u64) -> u128 {
    ((hi as u128) << 64) | lo as u128
}

pub fn unpack128(x: u128) -> [u64; 2] {
    [x as u64, (x >> 64) as u64]
}

#[cfg(test)]
#[path = "scalar.test.rs"]
mod test;
