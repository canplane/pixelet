// @xpute/spec/io/marshal/tlv.rs

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
//! - All scalar payloads use little-endian encoding.
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
//! - TlvWriter is append-only until finish(); finish() consumes the writer.
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

use crate::lib::kit::array::Bytes;
use crate::sys::errno;
use crate::sys::error::MarshalError;

// ============ ABI ============

// TLV tag domain includes both value tags and control tags (e.g. END)
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum Tag {
    End = 0x00, // END is a stream terminator control tag, not a payload element kind.

    Bool = 0x01, // [u8 0|1]

    U8  = 0x02,
    I8  = 0x03,
    U16 = 0x04,
    I16 = 0x05,
    U32 = 0x06,
    I32 = 0x07,
    U64 = 0x08,
    I64 = 0x09,
    F32 = 0x0a,
    F64 = 0x0b,

    Str   = 0x10, // [u32 len][utf8 bytes]
    Bytes = 0x20, // [u32 len][raw bytes], opaque binary blob (nonce/token/key/u8 payload)
}

#[derive(Clone, Debug)]
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
    Bytes(Bytes),
}

// ============ Writer ============

pub struct TlvWriter {
    buf: Vec<u8>,
}

impl TlvWriter {
    pub fn new() -> Self {
        Self { buf: Vec::with_capacity(1024) }
    }

    pub fn data(&self) -> &[u8] { &self.buf }

    /// Seal stream by appending END and return the buffer. Writer is consumed.
    pub fn finish(mut self) -> Bytes {
        self.buf.push(Tag::End as u8);
        self.buf
    }

    fn put_tag(&mut self, tag: Tag) { self.buf.push(tag as u8); }

    pub fn write_bool(&mut self, x: bool) -> &mut Self {
        self.put_tag(Tag::Bool);
        self.buf.push(x as u8);
        self
    }
    pub fn write_u8(&mut self, x: u8) -> &mut Self {
        self.put_tag(Tag::U8);
        self.buf.push(x);
        self
    }
    pub fn write_i8(&mut self, x: i8) -> &mut Self {
        self.put_tag(Tag::I8);
        self.buf.push(x as u8);
        self
    }
    pub fn write_u16(&mut self, x: u16) -> &mut Self {
        self.put_tag(Tag::U16);
        self.buf.extend_from_slice(&x.to_le_bytes());
        self
    }
    pub fn write_i16(&mut self, x: i16) -> &mut Self {
        self.put_tag(Tag::I16);
        self.buf.extend_from_slice(&x.to_le_bytes());
        self
    }
    pub fn write_u32(&mut self, x: u32) -> &mut Self {
        self.put_tag(Tag::U32);
        self.buf.extend_from_slice(&x.to_le_bytes());
        self
    }
    pub fn write_i32(&mut self, x: i32) -> &mut Self {
        self.put_tag(Tag::I32);
        self.buf.extend_from_slice(&x.to_le_bytes());
        self
    }
    pub fn write_u64(&mut self, x: u64) -> &mut Self {
        self.put_tag(Tag::U64);
        self.buf.extend_from_slice(&x.to_le_bytes());
        self
    }
    pub fn write_i64(&mut self, x: i64) -> &mut Self {
        self.put_tag(Tag::I64);
        self.buf.extend_from_slice(&x.to_le_bytes());
        self
    }
    pub fn write_f32(&mut self, x: f32) -> &mut Self {
        self.put_tag(Tag::F32);
        self.buf.extend_from_slice(&x.to_le_bytes());
        self
    }
    pub fn write_f64(&mut self, x: f64) -> &mut Self {
        self.put_tag(Tag::F64);
        self.buf.extend_from_slice(&x.to_le_bytes());
        self
    }
    pub fn write_str(&mut self, s: &str) -> &mut Self {
        let data = s.as_bytes();
        self.put_tag(Tag::Str);
        self.buf.extend_from_slice(&(data.len() as u32).to_le_bytes());
        self.buf.extend_from_slice(data);
        self
    }
    pub fn write_bytes(&mut self, b: &[u8]) -> &mut Self {
        self.put_tag(Tag::Bytes);
        self.buf.extend_from_slice(&(b.len() as u32).to_le_bytes());
        self.buf.extend_from_slice(b);
        self
    }

    /// Auto-dispatch write: matches on TlvValue variant.
    pub fn write_all(&mut self, vals: &[TlvValue]) -> &mut Self {
        for v in vals {
            match v {
                TlvValue::Bool(x)  => { self.write_bool(*x); }
                TlvValue::U8(x)    => { self.write_u8(*x); }
                TlvValue::I8(x)    => { self.write_i8(*x); }
                TlvValue::U16(x)   => { self.write_u16(*x); }
                TlvValue::I16(x)   => { self.write_i16(*x); }
                TlvValue::U32(x)   => { self.write_u32(*x); }
                TlvValue::I32(x)   => { self.write_i32(*x); }
                TlvValue::U64(x)   => { self.write_u64(*x); }
                TlvValue::I64(x)   => { self.write_i64(*x); }
                TlvValue::F32(x)   => { self.write_f32(*x); }
                TlvValue::F64(x)   => { self.write_f64(*x); }
                TlvValue::Str(s)   => { self.write_str(s); }
                TlvValue::Bytes(b) => { self.write_bytes(b); }
            }
        }
        self
    }
}

impl Default for TlvWriter {
    fn default() -> Self { Self::new() }
}

// ============ Reader ============

pub struct TlvReader {
    buf: Bytes,
    off: usize,
}

impl TlvReader {
    pub fn new(buf: Bytes) -> Self {
        Self { buf, off: 0 }
    }

    /// Materialize remaining stream into a Vec (until END or error).
    pub fn read_all(&mut self) -> Result<Vec<TlvValue>, MarshalError> {
        let mut out = Vec::new();
        loop {
            match self.decode_next()? {
                Some(v) => out.push(v),
                None    => return Ok(out),
            }
        }
    }

    fn need(&self, n: usize, what: &str) -> Result<(), MarshalError> {
        if self.off + n > self.buf.len() {
            Err(MarshalError::new(errno::EBADMSG, format!("TLV: truncated {} at {}", what, self.off)))
        } else {
            Ok(())
        }
    }

    fn get_u8(&mut self) -> u8  { let v = self.buf[self.off]; self.off += 1; v }
    fn get_i8(&mut self) -> i8  { self.get_u8() as i8 }

    fn get_u16(&mut self) -> u16 {
        let v = u16::from_le_bytes(self.buf[self.off..self.off + 2].try_into().unwrap());
        self.off += 2; v
    }
    fn get_i16(&mut self) -> i16 { self.get_u16() as i16 }

    fn get_u32(&mut self) -> u32 {
        let v = u32::from_le_bytes(self.buf[self.off..self.off + 4].try_into().unwrap());
        self.off += 4; v
    }
    fn get_i32(&mut self) -> i32 { self.get_u32() as i32 }

    fn get_u64(&mut self) -> u64 {
        let v = u64::from_le_bytes(self.buf[self.off..self.off + 8].try_into().unwrap());
        self.off += 8; v
    }
    fn get_i64(&mut self) -> i64 { self.get_u64() as i64 }

    fn get_f32(&mut self) -> f32 {
        let v = f32::from_le_bytes(self.buf[self.off..self.off + 4].try_into().unwrap());
        self.off += 4; v
    }
    fn get_f64(&mut self) -> f64 {
        let v = f64::from_le_bytes(self.buf[self.off..self.off + 8].try_into().unwrap());
        self.off += 8; v
    }

    fn decode_next(&mut self) -> Result<Option<TlvValue>, MarshalError> {
        if self.off >= self.buf.len() { return Ok(None); }
        self.need(1, "tag")?;
        let tag = self.get_u8();
        match tag {
            0x00 => Ok(None), // END
            0x01 => { self.need(1, "BOOL")?; Ok(Some(TlvValue::Bool(self.get_u8() != 0))) }
            0x02 => { self.need(1, "U8")?;   Ok(Some(TlvValue::U8(self.get_u8()))) }
            0x03 => { self.need(1, "I8")?;   Ok(Some(TlvValue::I8(self.get_i8()))) }
            0x04 => { self.need(2, "U16")?;  Ok(Some(TlvValue::U16(self.get_u16()))) }
            0x05 => { self.need(2, "I16")?;  Ok(Some(TlvValue::I16(self.get_i16()))) }
            0x06 => { self.need(4, "U32")?;  Ok(Some(TlvValue::U32(self.get_u32()))) }
            0x07 => { self.need(4, "I32")?;  Ok(Some(TlvValue::I32(self.get_i32()))) }
            0x08 => { self.need(8, "U64")?;  Ok(Some(TlvValue::U64(self.get_u64()))) }
            0x09 => { self.need(8, "I64")?;  Ok(Some(TlvValue::I64(self.get_i64()))) }
            0x0a => { self.need(4, "F32")?;  Ok(Some(TlvValue::F32(self.get_f32()))) }
            0x0b => { self.need(8, "F64")?;  Ok(Some(TlvValue::F64(self.get_f64()))) }
            0x10 => {
                self.need(4, "STR len")?;
                let len = self.get_u32() as usize;
                self.need(len, &format!("STR(len={})", len))?;
                let s = std::str::from_utf8(&self.buf[self.off..self.off + len])
                    .map_err(|_| MarshalError::new(errno::EBADMSG, format!("TLV: invalid UTF-8 in STR at {}", self.off)))?
                    .to_string();
                self.off += len;
                Ok(Some(TlvValue::Str(s)))
            }
            0x20 => {
                self.need(4, "BYTES len")?;
                let len = self.get_u32() as usize;
                self.need(len, &format!("BYTES(len={})", len))?;
                let b = self.buf[self.off..self.off + len].to_vec();
                self.off += len;
                Ok(Some(TlvValue::Bytes(b)))
            }
            other => Err(MarshalError::new(
                errno::EBADMSG,
                format!("TLV: unknown tag 0x{:02x} at {}", other, self.off - 1),
            )),
        }
    }
}

// Iterator surfaces errors as Err items; use read_all() for a collecting variant that short-circuits.
impl Iterator for TlvReader {
    type Item = Result<TlvValue, MarshalError>;

    fn next(&mut self) -> Option<Self::Item> {
        match self.decode_next() {
            Ok(Some(v)) => Some(Ok(v)),
            Ok(None)    => None,
            Err(e)      => Some(Err(e)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{TlvReader, TlvValue, TlvWriter};

    #[test]
    fn round_trip_values() {
        let values = vec![
            TlvValue::Bool(true),
            TlvValue::U8(7),
            TlvValue::I32(-42),
            TlvValue::U64(99),
            TlvValue::F64(3.5),
            TlvValue::Str("hello".to_string()),
            TlvValue::Bytes(vec![1, 2, 3]),
        ];

        let mut writer = TlvWriter::new();
        writer.write_all(&values);
        let bytes = writer.finish();

        let mut reader = TlvReader::new(bytes);
        let decoded = reader.read_all().expect("valid TLV");

        assert_eq!(decoded.len(), values.len());
        assert!(matches!(decoded[0], TlvValue::Bool(true)));
        assert!(matches!(decoded[1], TlvValue::U8(7)));
        assert!(matches!(decoded[2], TlvValue::I32(-42)));
        assert!(matches!(decoded[3], TlvValue::U64(99)));
        assert!(matches!(decoded[4], TlvValue::F64(x) if x == 3.5));
        assert!(matches!(decoded[5], TlvValue::Str(ref s) if s == "hello"));
        assert!(matches!(decoded[6], TlvValue::Bytes(ref b) if b == &vec![1, 2, 3]));
    }
}
