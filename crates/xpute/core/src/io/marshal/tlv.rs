// xpute-core/io/marshal/tlv.rs

//! TLV: self-delimiting binary field stream.
//!
//! Scope
//! -----
//! - This module encodes and decodes a generic sequential TLV stream.
//! - It is intentionally schema-light: meaning is assigned by higher layers.
//! - The stream is self-delimiting via END tag.
//! - This format is sequential and stream-oriented, not a relocatable object layout.
//!
//! Wire Layout
//! -----------
//! Repeated sequence of:
//!   [tag | payload]
//! terminated by:
//!   [END]
//!
//! Tag domain:
//! - u8 tag values
//! - tag space is format-owned
//! - END is a control tag, not a value payload kind
//!
//! Payload Layout
//! --------------
//! - BOOL  = [u8]
//! - U8    = [u8]
//! - I8    = [i8]
//! - U16   = [u16]
//! - I16   = [i16]
//! - U32   = [u32]
//! - I32   = [i32]
//! - U64   = [u64]
//! - I64   = [i64]
//! - F32   = [f32]
//! - F64   = [f64]
//! - STR   = [u32 len][utf8 bytes]
//! - BYTES = [u32 len][raw bytes]
//!
//! Endianness
//! ----------
//! - All scalar payloads use little-endian encoding via scalar.rs.
//!
//! Stream Semantics
//! ----------------
//! - END terminates the logical stream.
//! - Bytes after END are ignored by iteration semantics.
//! - Missing END is tolerated only if the buffer ends exactly after the last full element;
//!   truncation during element decode is an error.
//! - Elements are interpreted strictly in forward stream order.
//!
//! Reader / Writer Contract
//! ------------------------
//! - TLVWriter is append-only until finish(); write-after-finish is invalid.
//! - finish() appends END exactly once.
//! - TLVReader iterates elements sequentially from the start of the buffer.
//! - Unknown tag values are rejected with EBADMSG.
//! - Truncated payloads are rejected with EBADMSG.
//!
//! Value Model
//! -----------
//! - TLV is a stream format, not a random-access object layout.
//! - Repeated values are allowed.
//! - Field names, uniqueness, ordering requirements, and semantic constraints
//!   are entirely owned by higher layers.
//!
//! Non-Goals
//! ---------
//! - No schema, field table, or offset index.
//! - No nested container protocol in this base format.
//! - No checksum/hash/compression/encryption inside this format.
//! - No canonical integer width normalization beyond the explicit tag written.
//!
//! A value read back is a `TLVValue` with its tag kept: where the
//! TypeScript hands every integer width back as a `number` and the 64-bit
//! ones as a `bigint`, the enum says which tag it was. `Number` and
//! `BigInt` are the TypeScript's runtime shapes, for `write_all`'s
//! auto-dispatch.

use crate::io::scalar::{self, Stream};
use crate::kit::array::{Bytes, TypedArray};
use crate::kit::encoding;
use crate::sys::errno::Errno;
use crate::sys::error::MarshalError;

const U8_SZ: u32 = 1;
const U16_SZ: u32 = 2;
const U32_SZ: u32 = 4;
const U64_SZ: u32 = 8;

// ============ ABI ============

// ---- Elements ----

// TLV tag domain includes both value tags and control tags (e.g. END)
#[derive(Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
#[allow(clippy::upper_case_acronyms)]
enum Tag {
    END = 0x00, // END is a stream terminator control tag, not a payload element kind.

    BOOL = 0x01, // [u8 0|1]

    U8 = 0x02,
    I8 = 0x03,
    U16 = 0x04,
    I16 = 0x05,
    U32 = 0x06,
    I32 = 0x07,
    U64 = 0x08,
    I64 = 0x09,
    F32 = 0x0a,
    F64 = 0x0b,

    STR = 0x10, // [u32 len][utf8 bytes]

    BYTES = 0x20, // [u32 len][raw bytes], opaque binary blob (nonce/token/key/u8 payload)
}

#[derive(Clone, Debug, PartialEq)]
pub enum TLVValue {
    Bool(bool),
    U8(u8),
    I8(i8),
    U16(u16),
    I16(i16),
    U32(u32),
    I32(i32),
    U64(u64),
    I64(i64),
    F32(f32),
    F64(f64),
    /// A JS `number`, tagged by its shape and range on write.
    Number(f64),
    /// A JS `bigint`, written as I64.
    BigInt(i128),
    Str(String),
    Bytes(Bytes),
}

// ============ Tag I/O ============

// tag I/O is TLV-specific (not general scalar)
fn put_tag(s: &mut Stream, tag: Tag) {
    scalar::putu8(s, tag as u8)
}
fn get_tag(s: &mut Stream) -> u8 {
    scalar::getu8(s)
}

// ============ Writer ============

// ---- Writer core ----

pub struct TLVWriter {
    _buf: Bytes,
    _stream: Stream,
    _sealed: bool,
}

impl Default for TLVWriter {
    fn default() -> Self {
        TLVWriter::new(1024)
    }
}

impl TLVWriter {
    pub fn new(initial_cap: u32) -> TLVWriter {
        let _buf = TypedArray::<u8>::new(initial_cap as usize);
        let _stream = scalar::stream(_buf.clone(), 0);
        TLVWriter { _buf, _stream, _sealed: false }
    }

    fn _ensure(&mut self, size: u32) {
        if self._stream.off as usize + size as usize > self._buf.byte_length() {
            let new_cap = (self._buf.byte_length() * 2).max(self._stream.off as usize + size as usize);
            let new_buf: Bytes = TypedArray::new(new_cap);
            new_buf.set(&self._buf);
            self._buf = new_buf;

            // IMPORTANT: stream must be rebound to the new backing buffer.
            self._stream.buf = self._buf.clone();
            // offset stays valid (same numeric off)
        }
    }

    pub fn data(&self) -> Bytes {
        self._buf.subarray(0, self._stream.off as usize)
    }

    /// Seal stream by appending END. Writer must not be used after this.
    pub fn finish(&mut self) -> Bytes {
        if !self._sealed {
            self._ensure(1);
            put_tag(&mut self._stream, Tag::END);
            self._sealed = true;
        }
        self.data()
    }

    fn _check_unsealed(&self) -> Result<(), MarshalError> {
        if self._sealed {
            return Err(MarshalError::new(Errno::EBADMSG, Some("TLV: write after finish()"), None));
        }
        Ok(())
    }

    // ---- Primitive writers ----

    pub fn u8(&mut self, x: u8) -> Result<&mut Self, MarshalError> {
        self._check_unsealed()?;
        self._ensure(1 + U8_SZ);
        put_tag(&mut self._stream, Tag::U8);
        scalar::putu8(&mut self._stream, x);
        Ok(self)
    }
    pub fn i8(&mut self, x: i8) -> Result<&mut Self, MarshalError> {
        self._check_unsealed()?;
        self._ensure(1 + U8_SZ);
        put_tag(&mut self._stream, Tag::I8);
        scalar::puti8(&mut self._stream, x);
        Ok(self)
    }
    pub fn u16(&mut self, x: u16) -> Result<&mut Self, MarshalError> {
        self._check_unsealed()?;
        self._ensure(1 + U16_SZ);
        put_tag(&mut self._stream, Tag::U16);
        scalar::putu16(&mut self._stream, x);
        Ok(self)
    }
    pub fn i16(&mut self, x: i16) -> Result<&mut Self, MarshalError> {
        self._check_unsealed()?;
        self._ensure(1 + U16_SZ);
        put_tag(&mut self._stream, Tag::I16);
        scalar::puti16(&mut self._stream, x);
        Ok(self)
    }
    pub fn u32(&mut self, x: u32) -> Result<&mut Self, MarshalError> {
        self._check_unsealed()?;
        self._ensure(1 + U32_SZ);
        put_tag(&mut self._stream, Tag::U32);
        scalar::putu32(&mut self._stream, x);
        Ok(self)
    }
    pub fn i32(&mut self, x: i32) -> Result<&mut Self, MarshalError> {
        self._check_unsealed()?;
        self._ensure(1 + U32_SZ);
        put_tag(&mut self._stream, Tag::I32);
        scalar::puti32(&mut self._stream, x);
        Ok(self)
    }
    pub fn u64(&mut self, x: u64) -> Result<&mut Self, MarshalError> {
        self._check_unsealed()?;
        self._ensure(1 + U64_SZ);
        put_tag(&mut self._stream, Tag::U64);
        scalar::putu64(&mut self._stream, x);
        Ok(self)
    }
    pub fn i64(&mut self, x: i64) -> Result<&mut Self, MarshalError> {
        self._check_unsealed()?;
        self._ensure(1 + U64_SZ);
        put_tag(&mut self._stream, Tag::I64);
        scalar::puti64(&mut self._stream, x);
        Ok(self)
    }
    pub fn f32(&mut self, x: f32) -> Result<&mut Self, MarshalError> {
        self._check_unsealed()?;
        self._ensure(1 + U32_SZ);
        put_tag(&mut self._stream, Tag::F32);
        scalar::putf32(&mut self._stream, x);
        Ok(self)
    }
    pub fn f64(&mut self, x: f64) -> Result<&mut Self, MarshalError> {
        self._check_unsealed()?;
        self._ensure(1 + U64_SZ);
        put_tag(&mut self._stream, Tag::F64);
        scalar::putf64(&mut self._stream, x);
        Ok(self)
    }

    pub fn bool(&mut self, x: bool) -> Result<&mut Self, MarshalError> {
        self._check_unsealed()?;
        self._ensure(1 + U8_SZ);
        put_tag(&mut self._stream, Tag::BOOL);
        scalar::putu8(&mut self._stream, if x { 1 } else { 0 });
        Ok(self)
    }

    // ---- Ref writers ----

    pub fn str(&mut self, s: &str) -> Result<&mut Self, MarshalError> {
        self._check_unsealed()?;
        let data: Bytes = encoding::te.encode(s);
        let len = data.length() as u32;
        self._ensure(1 + U32_SZ + len);
        put_tag(&mut self._stream, Tag::STR);
        scalar::putu32(&mut self._stream, len);
        self._buf.set_at(&data, self._stream.off as usize);
        self._stream.off += len as i32;
        Ok(self)
    }

    pub fn bytes(&mut self, b: &Bytes) -> Result<&mut Self, MarshalError> {
        self._check_unsealed()?;
        let len = b.byte_length() as u32;
        self._ensure(1 + U32_SZ + len);
        put_tag(&mut self._stream, Tag::BYTES);
        scalar::putu32(&mut self._stream, len);
        self._buf.set_at(b, self._stream.off as usize);
        self._stream.off += len as i32;
        Ok(self)
    }

    // ---- Convenience writers ----

    // Auto-dispatch helper:
    // - Str -> STR
    // - Bytes -> BYTES
    // - BigInt/Number -> explicit integer/float tags by runtime shape/range
    pub fn write_all(&mut self, vals: &[TLVValue]) -> Result<&mut Self, MarshalError> {
        for v in vals {
            match v {
                TLVValue::Bool(x) => self.bool(*x)?,
                TLVValue::Number(v) => {
                    let v = *v;
                    if v.is_finite() && v.trunc() == v && v.abs() <= 9_007_199_254_740_991.0 {
                        if v >= i32::MIN as f64 && v <= i32::MAX as f64 {
                            self.i32(v as i32)?
                        } else {
                            self.i64(v as i64)?
                        }
                    } else {
                        // float path (includes NaN/Inf)
                        self.f64(v)?
                    }
                }
                TLVValue::BigInt(v) => {
                    if *v < i64::MIN as i128 || *v > i64::MAX as i128 {
                        return Err(MarshalError::new(Errno::EBADMSG, Some(&format!("TLV: bigint out of i64 range {v}")), None));
                    }
                    self.i64(*v as i64)?
                }
                TLVValue::Str(s) => self.str(s)?,
                TLVValue::Bytes(b) => self.bytes(b)?,
                TLVValue::U8(x) => self.u8(*x)?,
                TLVValue::I8(x) => self.i8(*x)?,
                TLVValue::U16(x) => self.u16(*x)?,
                TLVValue::I16(x) => self.i16(*x)?,
                TLVValue::U32(x) => self.u32(*x)?,
                TLVValue::I32(x) => self.i32(*x)?,
                TLVValue::U64(x) => self.u64(*x)?,
                TLVValue::I64(x) => self.i64(*x)?,
                TLVValue::F32(x) => self.f32(*x)?,
                TLVValue::F64(x) => self.f64(*x)?,
            };
        }
        Ok(self)
    }
}

// ============ Reader ============

// ---- Reader core ----

pub struct TLVReader {
    _buf: Bytes,
    _stream: Stream,
    _end: u32,
}

impl TLVReader {
    pub fn new(_buf: Bytes) -> TLVReader {
        let _stream = scalar::stream(_buf.clone(), 0);
        let _end = _buf.byte_length() as u32;
        TLVReader { _buf, _stream, _end }
    }

    /// Materialize the remaining stream into an array (until END).
    pub fn read_all(&mut self) -> Result<Vec<TLVValue>, MarshalError> {
        let mut out = Vec::new();
        while let Some(v) = self.next_value()? {
            out.push(v);
        }
        Ok(out)
    }

    /// The iterator's step: the next element, or none at END or the
    /// buffer's end.
    pub fn next_value(&mut self) -> Result<Option<TLVValue>, MarshalError> {
        if self._stream.off as u32 >= self._end {
            return Ok(None);
        }
        let tag = get_tag(&mut self._stream);
        if tag == Tag::END as u8 {
            return Ok(None);
        }

        Ok(Some(match tag {
            t if t == Tag::U8 as u8 => TLVValue::U8(self._u8()?),
            t if t == Tag::I8 as u8 => TLVValue::I8(self._i8()?),
            t if t == Tag::U16 as u8 => TLVValue::U16(self._u16()?),
            t if t == Tag::I16 as u8 => TLVValue::I16(self._i16()?),
            t if t == Tag::U32 as u8 => TLVValue::U32(self._u32()?),
            t if t == Tag::I32 as u8 => TLVValue::I32(self._i32()?),
            t if t == Tag::U64 as u8 => TLVValue::U64(self._u64()?),
            t if t == Tag::I64 as u8 => TLVValue::I64(self._i64()?),
            t if t == Tag::F32 as u8 => TLVValue::F32(self._f32()?),
            t if t == Tag::F64 as u8 => TLVValue::F64(self._f64()?),
            t if t == Tag::BOOL as u8 => TLVValue::Bool(self._bool()?),
            t if t == Tag::STR as u8 => TLVValue::Str(self._str()?),
            t if t == Tag::BYTES as u8 => TLVValue::Bytes(self._bytes()?),
            _ => return Err(MarshalError::new(Errno::EBADMSG, Some(&format!("TLV: unknown tag 0x{:x} at {}", tag, self._stream.off - 1)), None)),
        }))
    }

    // ---- Reader bounds ----

    fn _need(&self, n: u32, what: &str) -> Result<(), MarshalError> {
        if self._stream.off as u32 + n > self._end {
            return Err(MarshalError::new(Errno::EBADMSG, Some(&format!("TLV: truncated {what} at {}", self._stream.off)), None));
        }
        Ok(())
    }

    // ---- Primitive readers ----

    fn _u8(&mut self) -> Result<u8, MarshalError> {
        self._need(U8_SZ, "U8")?;
        Ok(scalar::getu8(&mut self._stream))
    }
    fn _i8(&mut self) -> Result<i8, MarshalError> {
        self._need(U8_SZ, "I8")?;
        Ok(scalar::geti8(&mut self._stream))
    }
    fn _u16(&mut self) -> Result<u16, MarshalError> {
        self._need(U16_SZ, "U16")?;
        Ok(scalar::getu16(&mut self._stream))
    }
    fn _i16(&mut self) -> Result<i16, MarshalError> {
        self._need(U16_SZ, "I16")?;
        Ok(scalar::geti16(&mut self._stream))
    }
    fn _u32(&mut self) -> Result<u32, MarshalError> {
        self._need(U32_SZ, "U32")?;
        Ok(scalar::getu32(&mut self._stream))
    }
    fn _i32(&mut self) -> Result<i32, MarshalError> {
        self._need(U32_SZ, "I32")?;
        Ok(scalar::geti32(&mut self._stream))
    }
    fn _u64(&mut self) -> Result<u64, MarshalError> {
        self._need(U64_SZ, "U64")?;
        Ok(scalar::getu64(&mut self._stream))
    }
    fn _i64(&mut self) -> Result<i64, MarshalError> {
        self._need(U64_SZ, "I64")?;
        Ok(scalar::geti64(&mut self._stream))
    }
    fn _f32(&mut self) -> Result<f32, MarshalError> {
        self._need(U32_SZ, "F32")?;
        Ok(scalar::getf32(&mut self._stream))
    }
    fn _f64(&mut self) -> Result<f64, MarshalError> {
        self._need(U64_SZ, "F64")?;
        Ok(scalar::getf64(&mut self._stream))
    }

    fn _bool(&mut self) -> Result<bool, MarshalError> {
        self._need(U8_SZ, "BOOL")?;
        Ok(scalar::getu8(&mut self._stream) != 0)
    }

    // ---- Ref readers ----

    fn _str(&mut self) -> Result<String, MarshalError> {
        let len = self._u32()?;
        self._need(len, &format!("STR(len={len})"))?;
        let s = encoding::td.decode(&self._buf.subarray(self._stream.off as usize, self._stream.off as usize + len as usize));
        self._stream.off += len as i32;
        Ok(s)
    }

    fn _bytes(&mut self) -> Result<Bytes, MarshalError> {
        let len = self._u32()?;
        self._need(len, &format!("BYTES(len={len})"))?;
        let b = self._buf.subarray(self._stream.off as usize, self._stream.off as usize + len as usize);
        self._stream.off += len as i32;
        Ok(b)
    }
}

impl Iterator for TLVReader {
    type Item = Result<TLVValue, MarshalError>;
    fn next(&mut self) -> Option<Self::Item> {
        self.next_value().transpose()
    }
}
