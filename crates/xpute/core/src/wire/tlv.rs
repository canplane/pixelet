// xpute-core/wire/tlv.rs

//! TLV: a forward-only sequence of `[tag u8 | payload]`, closed by END, all
//! little-endian. A tag fixes its payload's size; STR and BYTES lead with a
//! u32 length. Unlike XTP there is no table or offset: read once, in order.
//!
//! The reader reports how a sequence ended (`end`): closed by END, unclosed
//! at an element boundary, or cut (EBADMSG, the values before it stand).
//! Whether unclosed is acceptable is the caller's call.
//!
//! Equal values are equal bytes only under a fixed schema: 1 may be written
//! as a U8, a U32 or an F64.

use crate::status::errno::Errno;
use crate::status::error::MarshalError;

const U8_SZ: u32 = size_of::<u8>() as u32;
const U16_SZ: u32 = size_of::<u16>() as u32;
const U32_SZ: u32 = size_of::<u32>() as u32;
const U64_SZ: u32 = size_of::<u64>() as u32;

#[derive(Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
// The width tags are `U8`, `I16`, so the rest match them.
#[allow(clippy::upper_case_acronyms)]
enum Tag {
    END = 0x00,

    BOOL = 0x01,

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

    STR = 0x10,

    BYTES = 0x20,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TlvEnd {
    Closed,
    /// The input ran out at an element's boundary, without END.
    Unclosed,
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

pub struct TlvWriter {
    buf: Vec<u8>,
    sealed: bool,
}

impl Default for TlvWriter {
    fn default() -> Self {
        TlvWriter::new(1024)
    }
}

impl TlvWriter {
    pub fn new(initial_cap: u32) -> TlvWriter {
        TlvWriter {
            buf: Vec::with_capacity(initial_cap as usize),
            sealed: false,
        }
    }

    fn put(&mut self, tag: Tag, bytes: &[u8]) {
        self.buf.push(tag as u8);
        self.buf.extend_from_slice(bytes);
    }

    pub fn data(&self) -> &[u8] {
        &self.buf
    }

    /// Appends END once; any write after it fails.
    pub fn finish(&mut self) -> &[u8] {
        if !self.sealed {
            self.buf.push(Tag::END as u8);
            self.sealed = true;
        }
        self.data()
    }

    fn check_unsealed(&self) -> Result<(), MarshalError> {
        if self.sealed {
            return Err(MarshalError::new(Errno::EBADMSG));
        }
        Ok(())
    }

    pub fn u8(&mut self, x: u8) -> Result<&mut Self, MarshalError> {
        self.check_unsealed()?;
        self.put(Tag::U8, &x.to_le_bytes());
        Ok(self)
    }
    pub fn i8(&mut self, x: i8) -> Result<&mut Self, MarshalError> {
        self.check_unsealed()?;
        self.put(Tag::I8, &x.to_le_bytes());
        Ok(self)
    }
    pub fn u16(&mut self, x: u16) -> Result<&mut Self, MarshalError> {
        self.check_unsealed()?;
        self.put(Tag::U16, &x.to_le_bytes());
        Ok(self)
    }
    pub fn i16(&mut self, x: i16) -> Result<&mut Self, MarshalError> {
        self.check_unsealed()?;
        self.put(Tag::I16, &x.to_le_bytes());
        Ok(self)
    }
    pub fn u32(&mut self, x: u32) -> Result<&mut Self, MarshalError> {
        self.check_unsealed()?;
        self.put(Tag::U32, &x.to_le_bytes());
        Ok(self)
    }
    pub fn i32(&mut self, x: i32) -> Result<&mut Self, MarshalError> {
        self.check_unsealed()?;
        self.put(Tag::I32, &x.to_le_bytes());
        Ok(self)
    }
    pub fn u64(&mut self, x: u64) -> Result<&mut Self, MarshalError> {
        self.check_unsealed()?;
        self.put(Tag::U64, &x.to_le_bytes());
        Ok(self)
    }
    pub fn i64(&mut self, x: i64) -> Result<&mut Self, MarshalError> {
        self.check_unsealed()?;
        self.put(Tag::I64, &x.to_le_bytes());
        Ok(self)
    }
    pub fn f32(&mut self, x: f32) -> Result<&mut Self, MarshalError> {
        self.check_unsealed()?;
        self.put(Tag::F32, &x.to_le_bytes());
        Ok(self)
    }
    pub fn f64(&mut self, x: f64) -> Result<&mut Self, MarshalError> {
        self.check_unsealed()?;
        self.put(Tag::F64, &x.to_le_bytes());
        Ok(self)
    }

    pub fn bool(&mut self, x: bool) -> Result<&mut Self, MarshalError> {
        self.check_unsealed()?;
        self.put(Tag::BOOL, &[x as u8]);
        Ok(self)
    }

    pub fn str(&mut self, s: &str) -> Result<&mut Self, MarshalError> {
        self.check_unsealed()?;
        let data = s.as_bytes();
        self.put(Tag::STR, &(data.len() as u32).to_le_bytes());
        self.buf.extend_from_slice(data);
        Ok(self)
    }

    pub fn bytes(&mut self, b: &[u8]) -> Result<&mut Self, MarshalError> {
        self.check_unsealed()?;
        self.put(Tag::BYTES, &(b.len() as u32).to_le_bytes());
        self.buf.extend_from_slice(b);
        Ok(self)
    }

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

pub struct TlvReader<'a> {
    buf: &'a [u8],
    off: usize,
    end: Option<TlvEnd>,
}

impl<'a> TlvReader<'a> {
    pub fn new(buf: &'a [u8]) -> TlvReader<'a> {
        TlvReader { buf, off: 0, end: None }
    }

    /// None while values remain, and after a fault.
    pub fn end(&self) -> Option<TlvEnd> {
        self.end
    }

    fn take<const N: usize>(&mut self) -> [u8; N] {
        let at = self.off;
        self.off += N;
        self.buf[at..at + N].try_into().unwrap()
    }

    pub fn read_all(&mut self) -> Result<Vec<TlvValue>, MarshalError> {
        let mut out = Vec::new();
        while let Some(v) = self.next_value()? {
            out.push(v);
        }
        Ok(out)
    }

    pub fn next_value(&mut self) -> Result<Option<TlvValue>, MarshalError> {
        if self.end.is_some() {
            return Ok(None);
        }
        if self.off >= self.buf.len() {
            self.end = Some(TlvEnd::Unclosed);
            return Ok(None);
        }
        let tag = self.buf[self.off];
        self.off += 1;
        if tag == Tag::END as u8 {
            self.end = Some(TlvEnd::Closed);
            return Ok(None);
        }

        Ok(Some(match tag {
            t if t == Tag::U8 as u8 => TlvValue::U8(self.u8_of()?),
            t if t == Tag::I8 as u8 => TlvValue::I8(self.i8_of()?),
            t if t == Tag::U16 as u8 => TlvValue::U16(self.u16_of()?),
            t if t == Tag::I16 as u8 => TlvValue::I16(self.i16_of()?),
            t if t == Tag::U32 as u8 => TlvValue::U32(self.u32_of()?),
            t if t == Tag::I32 as u8 => TlvValue::I32(self.i32_of()?),
            t if t == Tag::U64 as u8 => TlvValue::U64(self.u64_of()?),
            t if t == Tag::I64 as u8 => TlvValue::I64(self.i64_of()?),
            t if t == Tag::F32 as u8 => TlvValue::F32(self.f32_of()?),
            t if t == Tag::F64 as u8 => TlvValue::F64(self.f64_of()?),
            t if t == Tag::BOOL as u8 => TlvValue::Bool(self.bool_of()?),
            t if t == Tag::STR as u8 => TlvValue::Str(self.str_of()?),
            t if t == Tag::BYTES as u8 => TlvValue::Bytes(self.bytes_of()?),
            _ => return Err(MarshalError::new(Errno::EBADMSG)),
        }))
    }

    #[track_caller]
    fn need(&self, n: u32) -> Result<(), MarshalError> {
        // In u64: a wire length near 2^32 would otherwise wrap under the end.
        if self.off as u64 + n as u64 > self.buf.len() as u64 {
            return Err(MarshalError::new(Errno::EBADMSG));
        }
        Ok(())
    }

    fn u8_of(&mut self) -> Result<u8, MarshalError> {
        self.need(U8_SZ)?;
        Ok(u8::from_le_bytes(self.take()))
    }
    fn i8_of(&mut self) -> Result<i8, MarshalError> {
        self.need(U8_SZ)?;
        Ok(i8::from_le_bytes(self.take()))
    }
    fn u16_of(&mut self) -> Result<u16, MarshalError> {
        self.need(U16_SZ)?;
        Ok(u16::from_le_bytes(self.take()))
    }
    fn i16_of(&mut self) -> Result<i16, MarshalError> {
        self.need(U16_SZ)?;
        Ok(i16::from_le_bytes(self.take()))
    }
    fn u32_of(&mut self) -> Result<u32, MarshalError> {
        self.need(U32_SZ)?;
        Ok(u32::from_le_bytes(self.take()))
    }
    fn i32_of(&mut self) -> Result<i32, MarshalError> {
        self.need(U32_SZ)?;
        Ok(i32::from_le_bytes(self.take()))
    }
    fn u64_of(&mut self) -> Result<u64, MarshalError> {
        self.need(U64_SZ)?;
        Ok(u64::from_le_bytes(self.take()))
    }
    fn i64_of(&mut self) -> Result<i64, MarshalError> {
        self.need(U64_SZ)?;
        Ok(i64::from_le_bytes(self.take()))
    }
    fn f32_of(&mut self) -> Result<f32, MarshalError> {
        self.need(U32_SZ)?;
        Ok(f32::from_le_bytes(self.take()))
    }
    fn f64_of(&mut self) -> Result<f64, MarshalError> {
        self.need(U64_SZ)?;
        Ok(f64::from_le_bytes(self.take()))
    }

    fn bool_of(&mut self) -> Result<bool, MarshalError> {
        self.need(U8_SZ)?;
        Ok(u8::from_le_bytes(self.take()) != 0)
    }

    fn str_of(&mut self) -> Result<String, MarshalError> {
        let len = self.u32_of()?;
        self.need(len)?;
        let at = self.off;
        self.off += len as usize;
        Ok(String::from_utf8_lossy(&self.buf[at..at + len as usize]).into_owned())
    }

    fn bytes_of(&mut self) -> Result<Vec<u8>, MarshalError> {
        let len = self.u32_of()?;
        self.need(len)?;
        let at = self.off;
        self.off += len as usize;
        Ok(self.buf[at..at + len as usize].to_vec())
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
