// xpute-core/wire/tlv.rs

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
//! - TlvWriter is append-only until finish(); write-after-finish is invalid.
//! - finish() appends END exactly once.
//! - TlvReader iterates elements sequentially from the start of the buffer.
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
//! A value read back is a `TlvValue`, one variant per tag: the width that
//! was written is the width that comes back, and `write_all` picks the
//! writer from the variant.

use crate::status::errno::Errno;
use crate::status::error::MarshalError;

const U8_SZ: u32 = size_of::<u8>() as u32;
const U16_SZ: u32 = size_of::<u16>() as u32;
const U32_SZ: u32 = size_of::<u32>() as u32;
const U64_SZ: u32 = size_of::<u64>() as u32;

// ============ ABI ============

// ---- Elements ----

// TLV tag domain includes both value tags and control tags (e.g. END)
#[derive(Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
// The widths are the format's names and cannot be anything but `U8` and
// `I16`, so the control tags are written the same way rather than the enum
// carrying two styles at once.
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
pub enum TlvValue {
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
    Str(String),
    Bytes(Vec<u8>),
}

// ============ Tag I/O ============

// ============ Writer ============

// ---- Writer core ----

pub struct TlvWriter {
    _buf: Vec<u8>,
    _sealed: bool,
}

impl Default for TlvWriter {
    fn default() -> Self {
        TlvWriter::new(1024)
    }
}

impl TlvWriter {
    pub fn new(initial_cap: u32) -> TlvWriter {
        TlvWriter {
            _buf: Vec::with_capacity(initial_cap as usize),
            _sealed: false,
        }
    }

    fn _put(&mut self, tag: Tag, bytes: &[u8]) {
        self._buf.push(tag as u8);
        self._buf.extend_from_slice(bytes);
    }

    pub fn data(&self) -> &[u8] {
        &self._buf
    }

    /// Seal stream by appending END. Writer must not be used after this.
    pub fn finish(&mut self) -> &[u8] {
        if !self._sealed {
            self._buf.push(Tag::END as u8);
            self._sealed = true;
        }
        self.data()
    }

    fn _check_unsealed(&self) -> Result<(), MarshalError> {
        if self._sealed {
            return Err(MarshalError::new(Errno::EBADMSG));
        }
        Ok(())
    }

    // ---- Primitive writers ----

    pub fn u8(&mut self, x: u8) -> Result<&mut Self, MarshalError> {
        self._check_unsealed()?;
        self._put(Tag::U8, &x.to_le_bytes());
        Ok(self)
    }
    pub fn i8(&mut self, x: i8) -> Result<&mut Self, MarshalError> {
        self._check_unsealed()?;
        self._put(Tag::I8, &x.to_le_bytes());
        Ok(self)
    }
    pub fn u16(&mut self, x: u16) -> Result<&mut Self, MarshalError> {
        self._check_unsealed()?;
        self._put(Tag::U16, &x.to_le_bytes());
        Ok(self)
    }
    pub fn i16(&mut self, x: i16) -> Result<&mut Self, MarshalError> {
        self._check_unsealed()?;
        self._put(Tag::I16, &x.to_le_bytes());
        Ok(self)
    }
    pub fn u32(&mut self, x: u32) -> Result<&mut Self, MarshalError> {
        self._check_unsealed()?;
        self._put(Tag::U32, &x.to_le_bytes());
        Ok(self)
    }
    pub fn i32(&mut self, x: i32) -> Result<&mut Self, MarshalError> {
        self._check_unsealed()?;
        self._put(Tag::I32, &x.to_le_bytes());
        Ok(self)
    }
    pub fn u64(&mut self, x: u64) -> Result<&mut Self, MarshalError> {
        self._check_unsealed()?;
        self._put(Tag::U64, &x.to_le_bytes());
        Ok(self)
    }
    pub fn i64(&mut self, x: i64) -> Result<&mut Self, MarshalError> {
        self._check_unsealed()?;
        self._put(Tag::I64, &x.to_le_bytes());
        Ok(self)
    }
    pub fn f32(&mut self, x: f32) -> Result<&mut Self, MarshalError> {
        self._check_unsealed()?;
        self._put(Tag::F32, &x.to_le_bytes());
        Ok(self)
    }
    pub fn f64(&mut self, x: f64) -> Result<&mut Self, MarshalError> {
        self._check_unsealed()?;
        self._put(Tag::F64, &x.to_le_bytes());
        Ok(self)
    }

    pub fn bool(&mut self, x: bool) -> Result<&mut Self, MarshalError> {
        self._check_unsealed()?;
        self._put(Tag::BOOL, &[x as u8]);
        Ok(self)
    }

    // ---- Ref writers ----

    pub fn str(&mut self, s: &str) -> Result<&mut Self, MarshalError> {
        self._check_unsealed()?;
        let data = s.as_bytes();
        self._put(Tag::STR, &(data.len() as u32).to_le_bytes());
        self._buf.extend_from_slice(data);
        Ok(self)
    }

    pub fn bytes(&mut self, b: &[u8]) -> Result<&mut Self, MarshalError> {
        self._check_unsealed()?;
        self._put(Tag::BYTES, &(b.len() as u32).to_le_bytes());
        self._buf.extend_from_slice(b);
        Ok(self)
    }

    // ---- Convenience writers ----

    pub fn write_all(&mut self, vals: &[TlvValue]) -> Result<&mut Self, MarshalError> {
        for v in vals {
            match v {
                TlvValue::Bool(x) => self.bool(*x)?,
                TlvValue::Str(s) => self.str(s)?,
                TlvValue::Bytes(b) => self.bytes(b)?,
                TlvValue::U8(x) => self.u8(*x)?,
                TlvValue::I8(x) => self.i8(*x)?,
                TlvValue::U16(x) => self.u16(*x)?,
                TlvValue::I16(x) => self.i16(*x)?,
                TlvValue::U32(x) => self.u32(*x)?,
                TlvValue::I32(x) => self.i32(*x)?,
                TlvValue::U64(x) => self.u64(*x)?,
                TlvValue::I64(x) => self.i64(*x)?,
                TlvValue::F32(x) => self.f32(*x)?,
                TlvValue::F64(x) => self.f64(*x)?,
            };
        }
        Ok(self)
    }
}

// ============ Reader ============

// ---- Reader core ----

pub struct TlvReader<'a> {
    _buf: &'a [u8],
    _off: usize,
}

impl<'a> TlvReader<'a> {
    pub fn new(_buf: &'a [u8]) -> TlvReader<'a> {
        TlvReader { _buf, _off: 0 }
    }

    fn _take<const N: usize>(&mut self) -> [u8; N] {
        let at = self._off;
        self._off += N;
        self._buf[at..at + N].try_into().unwrap()
    }

    /// Materialize the remaining stream into an array (until END).
    pub fn read_all(&mut self) -> Result<Vec<TlvValue>, MarshalError> {
        let mut out = Vec::new();
        while let Some(v) = self.next_value()? {
            out.push(v);
        }
        Ok(out)
    }

    /// The iterator's step: the next element, or none at END or the
    /// buffer's end.
    pub fn next_value(&mut self) -> Result<Option<TlvValue>, MarshalError> {
        if self._off >= self._buf.len() {
            return Ok(None);
        }
        let tag = self._buf[self._off];
        self._off += 1;
        if tag == Tag::END as u8 {
            return Ok(None);
        }

        Ok(Some(match tag {
            t if t == Tag::U8 as u8 => TlvValue::U8(self._u8()?),
            t if t == Tag::I8 as u8 => TlvValue::I8(self._i8()?),
            t if t == Tag::U16 as u8 => TlvValue::U16(self._u16()?),
            t if t == Tag::I16 as u8 => TlvValue::I16(self._i16()?),
            t if t == Tag::U32 as u8 => TlvValue::U32(self._u32()?),
            t if t == Tag::I32 as u8 => TlvValue::I32(self._i32()?),
            t if t == Tag::U64 as u8 => TlvValue::U64(self._u64()?),
            t if t == Tag::I64 as u8 => TlvValue::I64(self._i64()?),
            t if t == Tag::F32 as u8 => TlvValue::F32(self._f32()?),
            t if t == Tag::F64 as u8 => TlvValue::F64(self._f64()?),
            t if t == Tag::BOOL as u8 => TlvValue::Bool(self._bool()?),
            t if t == Tag::STR as u8 => TlvValue::Str(self._str()?),
            t if t == Tag::BYTES as u8 => TlvValue::Bytes(self._bytes()?),
            _ => return Err(MarshalError::new(Errno::EBADMSG)),
        }))
    }

    // ---- Reader bounds ----

    fn _need(&self, n: u32) -> Result<(), MarshalError> {
        // In u64: `n` is a length read off the wire, and the sum past 2^32
        // would wrap under the end rather than exceed it.
        if self._off as u64 + n as u64 > self._buf.len() as u64 {
            return Err(MarshalError::new(Errno::EBADMSG));
        }
        Ok(())
    }

    // ---- Primitive readers ----

    fn _u8(&mut self) -> Result<u8, MarshalError> {
        self._need(U8_SZ)?;
        Ok(u8::from_le_bytes(self._take()))
    }
    fn _i8(&mut self) -> Result<i8, MarshalError> {
        self._need(U8_SZ)?;
        Ok(i8::from_le_bytes(self._take()))
    }
    fn _u16(&mut self) -> Result<u16, MarshalError> {
        self._need(U16_SZ)?;
        Ok(u16::from_le_bytes(self._take()))
    }
    fn _i16(&mut self) -> Result<i16, MarshalError> {
        self._need(U16_SZ)?;
        Ok(i16::from_le_bytes(self._take()))
    }
    fn _u32(&mut self) -> Result<u32, MarshalError> {
        self._need(U32_SZ)?;
        Ok(u32::from_le_bytes(self._take()))
    }
    fn _i32(&mut self) -> Result<i32, MarshalError> {
        self._need(U32_SZ)?;
        Ok(i32::from_le_bytes(self._take()))
    }
    fn _u64(&mut self) -> Result<u64, MarshalError> {
        self._need(U64_SZ)?;
        Ok(u64::from_le_bytes(self._take()))
    }
    fn _i64(&mut self) -> Result<i64, MarshalError> {
        self._need(U64_SZ)?;
        Ok(i64::from_le_bytes(self._take()))
    }
    fn _f32(&mut self) -> Result<f32, MarshalError> {
        self._need(U32_SZ)?;
        Ok(f32::from_le_bytes(self._take()))
    }
    fn _f64(&mut self) -> Result<f64, MarshalError> {
        self._need(U64_SZ)?;
        Ok(f64::from_le_bytes(self._take()))
    }

    fn _bool(&mut self) -> Result<bool, MarshalError> {
        self._need(U8_SZ)?;
        Ok(u8::from_le_bytes(self._take()) != 0)
    }

    // ---- Ref readers ----

    fn _str(&mut self) -> Result<String, MarshalError> {
        let len = self._u32()?;
        self._need(len)?;
        let at = self._off;
        self._off += len as usize;
        Ok(String::from_utf8_lossy(&self._buf[at..at + len as usize]).into_owned())
    }

    fn _bytes(&mut self) -> Result<Vec<u8>, MarshalError> {
        let len = self._u32()?;
        self._need(len)?;
        let at = self._off;
        self._off += len as usize;
        Ok(self._buf[at..at + len as usize].to_vec())
    }
}

impl Iterator for TlvReader<'_> {
    type Item = Result<TlvValue, MarshalError>;
    fn next(&mut self) -> Option<Self::Item> {
        self.next_value().transpose()
    }
}

#[cfg(test)]
#[path = "tlv.test.rs"]
mod test;
