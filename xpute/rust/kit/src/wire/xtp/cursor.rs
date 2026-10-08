// xpute-kit/wire/xtp/cursor.rs

//! The read end: lazy cursors over a packet. Nothing is decoded until a `get`.

// Stateless helpers are uppercase, as the C header's macros they mirror.
#![allow(non_snake_case)]

use crate::abi::word::field_get32;
use crate::status::errno::Errno;
use crate::status::error::MarshalError;

use super::spec::{AlignUnit, Array, NodeType, NodeValue, ScalarType, SequenceType, SpecialType, ALIGN_SZ, DESC_TYPE_MASK, DESC_TYPE_SHAMT, GET_WORD, HDR_SZ, MAGIC, NONE, TYPE_IS_LEAF, WORD_SZ};

pub type ShallowNodeValue<'a> = NodeValue<'a, BranchCursor<'a>>;

const WORD_REG: [u32; 2] = [NONE, NONE];

fn CURSOR(pkt: &[u8], base: u32, type_: NodeType) -> Result<Cursor<'_>, MarshalError> {
    let align_sz: AlignUnit = ALIGN_SZ(type_)?;
    if !base.is_multiple_of(align_sz) {
        return Err(MarshalError::new(Errno::EBADMSG));
    }
    if type_ == SpecialType::BRANCH as u8 {
        return Ok(Cursor::Branch(BranchCursor::new(pkt, base)?));
    }
    if TYPE_IS_LEAF(type_) {
        return Ok(Cursor::Node(NodeCursor::new(pkt, base, type_)));
    }
    Err(MarshalError::new(Errno::EBADMSG))
}

fn ENTRY_OFF(base: u32, idx: u32) -> u32 {
    (base + WORD_SZ) + idx * WORD_SZ
}

/// `Null` is a physically absent node that keeps its type.
#[derive(Clone, Debug)]
pub enum Cursor<'a> {
    Node(NodeCursor<'a>),
    Null(NullCursor<'a>),
    Branch(BranchCursor<'a>),
}

impl<'a> Cursor<'a> {
    pub fn type_(&self) -> NodeType {
        match self {
            Cursor::Node(c) => c.type_,
            Cursor::Null(c) => c.0.type_,
            Cursor::Branch(c) => c.cursor.type_,
        }
    }

    pub fn is_branch(&self) -> bool {
        matches!(self, Cursor::Branch(_))
    }

    pub fn as_branch(self) -> Result<BranchCursor<'a>, MarshalError> {
        match self {
            Cursor::Branch(b) => Ok(b),
            Cursor::Null(n) => n.as_branch(),
            Cursor::Node(c) => c.as_branch(),
        }
    }

    pub fn opt(self) -> Option<Cursor<'a>> {
        match self {
            Cursor::Null(_) => None,
            c => Some(c),
        }
    }

    pub fn get(&self) -> Result<ShallowNodeValue<'a>, MarshalError> {
        match self {
            Cursor::Node(c) => c.get(),
            Cursor::Null(c) => Ok(c.get()),
            Cursor::Branch(c) => c.get(),
        }
    }

    /// An array of another element type is EBADMSG, never reinterpreted.
    pub fn get_array<T: ArrayElem>(&self) -> Result<&'a [T], MarshalError> {
        match self {
            Cursor::Node(c) if c.type_ == T::TYPE as u8 => c.array::<T>(),
            _ => Err(MarshalError::new(Errno::EBADMSG)),
        }
    }

    pub fn get_u64(&self) -> Result<u64, MarshalError> {
        match self.get()? {
            NodeValue::U64(x) => Ok(x),
            _ => Err(MarshalError::new(Errno::EBADMSG)),
        }
    }

    pub fn get_i64(&self) -> Result<i64, MarshalError> {
        match self.get()? {
            NodeValue::I64(x) => Ok(x),
            _ => Err(MarshalError::new(Errno::EBADMSG)),
        }
    }

    pub fn get_bool(&self) -> Result<bool, MarshalError> {
        match self.get()? {
            NodeValue::Bool(b) => Ok(b),
            _ => Err(MarshalError::new(Errno::EBADMSG)),
        }
    }

    pub fn get_str(&self) -> Result<String, MarshalError> {
        match self.get()? {
            NodeValue::Str(s) => Ok(s),
            _ => Err(MarshalError::new(Errno::EBADMSG)),
        }
    }

    pub fn get_strs(&self) -> Result<Strs<'a>, MarshalError> {
        match self {
            Cursor::Node(c) if c.type_ == SequenceType::STRS as u8 => c.strs(),
            _ => Err(MarshalError::new(Errno::EBADMSG)),
        }
    }

    /// Invalid UTF-8 is EBADMSG here, where `get_str` replaces it.
    pub fn get_str_ref(&self) -> Result<&'a str, MarshalError> {
        match self {
            Cursor::Node(c) if c.type_ == SequenceType::STR as u8 => c.str_ref(),
            _ => Err(MarshalError::new(Errno::EBADMSG)),
        }
    }

    /// Only types exact in an f64; 64-bit integers are refused.
    pub fn get_number(&self) -> Result<f64, MarshalError> {
        Ok(match self.get()? {
            NodeValue::U8(v) => v as f64,
            NodeValue::I8(v) => v as f64,
            NodeValue::U16(v) => v as f64,
            NodeValue::I16(v) => v as f64,
            NodeValue::U32(v) => v as f64,
            NodeValue::I32(v) => v as f64,
            NodeValue::F32(v) => v as f64,
            NodeValue::F64(v) => v,
            _ => return Err(MarshalError::new(Errno::EBADMSG)),
        })
    }

    pub fn get_deep(&self) -> Result<NodeValue<'a>, MarshalError> {
        match self {
            Cursor::Node(c) => Ok(deepen(c.get()?)),
            Cursor::Null(c) => Ok(deepen(c.get())),
            Cursor::Branch(c) => c.get_deep(),
        }
    }
}

fn deepen(v: ShallowNodeValue<'_>) -> NodeValue<'_> {
    match v {
        NodeValue::Extra(_) => crate::bug!(ENOTRECOVERABLE),
        NodeValue::Null => NodeValue::Null,
        NodeValue::Bool(b) => NodeValue::Bool(b),
        NodeValue::U8(n) => NodeValue::U8(n),
        NodeValue::I8(n) => NodeValue::I8(n),
        NodeValue::U16(n) => NodeValue::U16(n),
        NodeValue::I16(n) => NodeValue::I16(n),
        NodeValue::U32(n) => NodeValue::U32(n),
        NodeValue::I32(n) => NodeValue::I32(n),
        NodeValue::F32(n) => NodeValue::F32(n),
        NodeValue::F64(n) => NodeValue::F64(n),
        NodeValue::U64(n) => NodeValue::U64(n),
        NodeValue::I64(n) => NodeValue::I64(n),
        NodeValue::Str(s) => NodeValue::Str(s),
        NodeValue::Array(a) => NodeValue::Array(a),
        NodeValue::List(l) => NodeValue::List(l.into_iter().map(deepen).collect()),
    }
}

/// The packet must start WORD_SZ-aligned.
pub struct TreeReader<'a> {
    pub pkt: &'a [u8],

    pub root: Cursor<'a>,
}

impl<'a> TreeReader<'a> {
    pub fn new(pkt: &'a [u8]) -> Result<TreeReader<'a>, MarshalError> {
        let base = pkt.as_ptr() as usize;
        if !base.is_multiple_of(WORD_SZ as usize) {
            return Err(MarshalError::new(Errno::EBADMSG));
        }

        if (pkt.len() as u32) < HDR_SZ {
            return Err(MarshalError::new(Errno::EBADMSG));
        }
        let hdr_view = &pkt[..HDR_SZ as usize];

        let mut reg = WORD_REG;
        let [magic, _] = GET_WORD(hdr_view, 0, &mut reg);
        if magic != MAGIC {
            return Err(MarshalError::new(Errno::EBADMSG));
        }

        // The root's word holds the payload size, not a relative offset.
        let [payload_sz, desc] = GET_WORD(hdr_view, WORD_SZ, &mut reg);
        if payload_sz > pkt.len() as u32 - HDR_SZ {
            return Err(MarshalError::new(Errno::EBADMSG));
        }
        let type_: NodeType = field_get32(desc, DESC_TYPE_SHAMT, DESC_TYPE_MASK) as NodeType;

        let root = if payload_sz != 0 {
            CURSOR(pkt, HDR_SZ, type_)?
        } else {
            Cursor::Null(NullCursor::new(pkt, type_))
        };
        Ok(TreeReader { pkt, root })
    }

    pub fn read(&self) -> Cursor<'a> {
        self.root.clone()
    }

    pub fn read_branch(&self) -> Result<BranchCursor<'a>, MarshalError> {
        self.root.clone().as_branch()
    }
}

#[derive(Clone, Debug)]
pub struct NodeCursor<'a> {
    pub pkt: &'a [u8],
    pub base: u32,

    pub type_: NodeType,
}

impl<'a> NodeCursor<'a> {
    pub fn new(pkt: &'a [u8], base: u32, type_: NodeType) -> NodeCursor<'a> {
        NodeCursor { pkt, base, type_ }
    }

    pub fn is_branch(&self) -> bool {
        self.type_ == SpecialType::BRANCH as u8
    }

    pub fn as_branch(self) -> Result<BranchCursor<'a>, MarshalError> {
        if self.type_ != SpecialType::BRANCH as u8 {
            return Err(MarshalError::new(Errno::EBADMSG));
        }
        BranchCursor::new(self.pkt, self.base)
    }

    pub fn opt(self) -> Option<NodeCursor<'a>> {
        Some(self)
    }

    pub fn get(&self) -> Result<ShallowNodeValue<'a>, MarshalError> {
        let t = self.type_;
        Ok(match t {
            t if t == ScalarType::U8 as u8 => NodeValue::U8(self.u8_of()?),
            t if t == ScalarType::I8 as u8 => NodeValue::I8(self.i8_of()?),
            t if t == ScalarType::U16 as u8 => NodeValue::U16(self.u16_of()?),
            t if t == ScalarType::I16 as u8 => NodeValue::I16(self.i16_of()?),
            t if t == ScalarType::U32 as u8 => NodeValue::U32(self.u32_of()?),
            t if t == ScalarType::I32 as u8 => NodeValue::I32(self.i32_of()?),
            t if t == ScalarType::U64 as u8 => NodeValue::U64(self.u64_of()?),
            t if t == ScalarType::I64 as u8 => NodeValue::I64(self.i64_of()?),
            t if t == ScalarType::F32 as u8 => NodeValue::F32(self.f32_of()?),
            t if t == ScalarType::F64 as u8 => NodeValue::F64(self.f64_of()?),
            t if t == ScalarType::BOOL as u8 => NodeValue::Bool(self.bool_of()?),

            t if t == SequenceType::U8_ARRAY as u8 => NodeValue::Array(Array {
                type_: SequenceType::U8_ARRAY,
                bytes: bytes_of(self.u8_array()?).into(),
            }),
            t if t == SequenceType::I8_ARRAY as u8 => NodeValue::Array(Array {
                type_: SequenceType::I8_ARRAY,
                bytes: bytes_of(self.i8_array()?).into(),
            }),
            t if t == SequenceType::U16_ARRAY as u8 => NodeValue::Array(Array {
                type_: SequenceType::U16_ARRAY,
                bytes: bytes_of(self.u16_array()?).into(),
            }),
            t if t == SequenceType::I16_ARRAY as u8 => NodeValue::Array(Array {
                type_: SequenceType::I16_ARRAY,
                bytes: bytes_of(self.i16_array()?).into(),
            }),
            t if t == SequenceType::U32_ARRAY as u8 => NodeValue::Array(Array {
                type_: SequenceType::U32_ARRAY,
                bytes: bytes_of(self.u32_array()?).into(),
            }),
            t if t == SequenceType::I32_ARRAY as u8 => NodeValue::Array(Array {
                type_: SequenceType::I32_ARRAY,
                bytes: bytes_of(self.i32_array()?).into(),
            }),
            t if t == SequenceType::U64_ARRAY as u8 => NodeValue::Array(Array {
                type_: SequenceType::U64_ARRAY,
                bytes: bytes_of(self.u64_array()?).into(),
            }),
            t if t == SequenceType::I64_ARRAY as u8 => NodeValue::Array(Array {
                type_: SequenceType::I64_ARRAY,
                bytes: bytes_of(self.i64_array()?).into(),
            }),
            t if t == SequenceType::F32_ARRAY as u8 => NodeValue::Array(Array {
                type_: SequenceType::F32_ARRAY,
                bytes: bytes_of(self.f32_array()?).into(),
            }),
            t if t == SequenceType::F64_ARRAY as u8 => NodeValue::Array(Array {
                type_: SequenceType::F64_ARRAY,
                bytes: bytes_of(self.f64_array()?).into(),
            }),
            t if t == SequenceType::BITSET as u8 => NodeValue::Array(Array {
                type_: SequenceType::BITSET,
                bytes: self.bitset()?.into(),
            }),
            t if t == SequenceType::STR as u8 => NodeValue::Str(self.str_of()?),
            t if t == SequenceType::STRS as u8 => NodeValue::List(self.strs()?.iter().map(|s| NodeValue::Str(s.to_string())).collect()),

            _ => return Err(MarshalError::new(Errno::EBADMSG)),
        })
    }

    fn scalar<const N: usize>(&self) -> Result<[u8; N], MarshalError> {
        if self.base as usize + N > self.pkt.len() {
            return Err(MarshalError::new(Errno::EBADMSG));
        }
        Ok(self.pkt[self.base as usize..self.base as usize + N].try_into().unwrap())
    }

    fn u8_of(&self) -> Result<u8, MarshalError> {
        Ok(u8::from_le_bytes(self.scalar()?))
    }
    fn i8_of(&self) -> Result<i8, MarshalError> {
        Ok(i8::from_le_bytes(self.scalar()?))
    }
    fn u16_of(&self) -> Result<u16, MarshalError> {
        Ok(u16::from_le_bytes(self.scalar()?))
    }
    fn i16_of(&self) -> Result<i16, MarshalError> {
        Ok(i16::from_le_bytes(self.scalar()?))
    }
    fn u32_of(&self) -> Result<u32, MarshalError> {
        Ok(u32::from_le_bytes(self.scalar()?))
    }
    fn i32_of(&self) -> Result<i32, MarshalError> {
        Ok(i32::from_le_bytes(self.scalar()?))
    }
    fn u64_of(&self) -> Result<u64, MarshalError> {
        Ok(u64::from_le_bytes(self.scalar()?))
    }
    fn i64_of(&self) -> Result<i64, MarshalError> {
        Ok(i64::from_le_bytes(self.scalar()?))
    }
    fn f32_of(&self) -> Result<f32, MarshalError> {
        Ok(f32::from_le_bytes(self.scalar()?))
    }
    fn f64_of(&self) -> Result<f64, MarshalError> {
        Ok(f64::from_le_bytes(self.scalar()?))
    }
    fn bool_of(&self) -> Result<bool, MarshalError> {
        Ok(u8::from_le_bytes(self.scalar()?) != 0)
    }

    /// Zero-copy; relies on `TreeReader` having checked the packet's alignment.
    fn array<T: Copy>(&self) -> Result<&'a [T], MarshalError> {
        if self.base + WORD_SZ > self.pkt.len() as u32 {
            return Err(MarshalError::new(Errno::EBADMSG));
        }
        let mut reg = WORD_REG;
        let [len, _] = GET_WORD(self.pkt, self.base, &mut reg);
        let payload_start = self.base + WORD_SZ;

        let elem_sz = core::mem::size_of::<T>() as u32;
        let payload_sz = len.wrapping_mul(elem_sz);

        if len != 0 && payload_sz / elem_sz != len {
            return Err(MarshalError::new(Errno::EBADMSG));
        }

        if payload_start > self.pkt.len() as u32 {
            return Err(MarshalError::new(Errno::EBADMSG));
        }
        if payload_sz > self.pkt.len() as u32 - payload_start {
            return Err(MarshalError::new(Errno::EBADMSG));
        }

        let at = payload_start as usize;
        // SAFETY: the payload is WORD_SZ-aligned, enough for every element
        // type (8 bytes at most), and the bounds above keep it in the packet.
        Ok(unsafe { core::slice::from_raw_parts(self.pkt[at..].as_ptr() as *const T, len as usize) })
    }

    fn u8_array(&self) -> Result<&'a [u8], MarshalError> {
        self.array()
    }
    fn i8_array(&self) -> Result<&'a [i8], MarshalError> {
        self.array()
    }
    fn u16_array(&self) -> Result<&'a [u16], MarshalError> {
        self.array()
    }
    fn i16_array(&self) -> Result<&'a [i16], MarshalError> {
        self.array()
    }
    fn u32_array(&self) -> Result<&'a [u32], MarshalError> {
        self.array()
    }
    fn i32_array(&self) -> Result<&'a [i32], MarshalError> {
        self.array()
    }
    fn u64_array(&self) -> Result<&'a [u64], MarshalError> {
        self.array()
    }
    fn i64_array(&self) -> Result<&'a [i64], MarshalError> {
        self.array()
    }
    fn f32_array(&self) -> Result<&'a [f32], MarshalError> {
        self.array()
    }
    fn f64_array(&self) -> Result<&'a [f64], MarshalError> {
        self.array()
    }

    fn bitset(&self) -> Result<Vec<u8>, MarshalError> {
        if self.base + WORD_SZ > self.pkt.len() as u32 {
            return Err(MarshalError::new(Errno::EBADMSG));
        }
        let mut reg = WORD_REG;
        let [len, _] = GET_WORD(self.pkt, self.base, &mut reg);
        let payload_start = self.base + WORD_SZ;

        // A wire length: u64, so a count near 2^32 cannot wrap to a small size.
        let payload_sz = (len as u64 + 7) >> 3;
        if payload_start > self.pkt.len() as u32 {
            return Err(MarshalError::new(Errno::EBADMSG));
        }
        if payload_sz > (self.pkt.len() as u32 - payload_start) as u64 {
            return Err(MarshalError::new(Errno::EBADMSG));
        }

        let mut arr = vec![0u8; len as usize];
        for (i, slot) in arr.iter_mut().enumerate() {
            let x = self.pkt[payload_start as usize + (i >> 3)] & (1 << (i & 7));
            *slot = (x != 0) as u8;
        }
        Ok(arr)
    }

    fn str_ref(&self) -> Result<&'a str, MarshalError> {
        if self.base + WORD_SZ > self.pkt.len() as u32 {
            return Err(MarshalError::new(Errno::EBADMSG));
        }
        let mut reg = WORD_REG;
        let [nbyte, _] = GET_WORD(self.pkt, self.base, &mut reg);
        let payload_start = self.base + WORD_SZ;
        if payload_start > self.pkt.len() as u32 || nbyte as u64 + 1 > (self.pkt.len() as u32 - payload_start) as u64 {
            return Err(MarshalError::new(Errno::EBADMSG));
        }
        let bytes = self.pkt;
        if bytes[(payload_start + nbyte) as usize] != 0 {
            return Err(MarshalError::new(Errno::EBADMSG));
        }
        core::str::from_utf8(&bytes[payload_start as usize..(payload_start + nbyte) as usize]).map_err(|_| MarshalError::new(Errno::EBADMSG))
    }

    /// `len` + 1 non-decreasing offsets into the UTF-8 after them; every
    /// string is checked before any is given.
    fn strs(&self) -> Result<Strs<'a>, MarshalError> {
        let bad = || MarshalError::new(Errno::EBADMSG);
        if self.base + WORD_SZ > self.pkt.len() as u32 {
            return Err(bad());
        }
        let mut reg = WORD_REG;
        let [n, _] = GET_WORD(self.pkt, self.base, &mut reg);
        let start = (self.base + WORD_SZ) as usize;
        let pkt: &'a [u8] = self.pkt;
        // In u64: a count a packet cannot hold must read as one, not wrap.
        let lane = 4 * (n as u64 + 1);
        if start as u64 + lane > pkt.len() as u64 {
            return Err(bad());
        }
        let offs = &pkt[start..start + lane as usize];
        let strs = Strs { offs, blob: &[], len: n };
        let blob = start + offs.len();
        let total = strs.off(n as usize);
        let strs = Strs {
            blob: pkt.get(blob..blob.checked_add(total).ok_or_else(bad)?).ok_or_else(bad)?,
            ..strs
        };
        for i in 0..n as usize {
            let (a, b) = (strs.off(i), strs.off(i + 1));
            if a > b || b > total || core::str::from_utf8(&strs.blob[a..b]).is_err() {
                return Err(bad());
            }
        }
        Ok(strs)
    }

    // On the wire a STR has a trailing NUL that `len` excludes.
    fn str_of(&self) -> Result<String, MarshalError> {
        if self.base + WORD_SZ > self.pkt.len() as u32 {
            return Err(MarshalError::new(Errno::EBADMSG));
        }
        let mut reg = WORD_REG;
        let [nbyte, _] = GET_WORD(self.pkt, self.base, &mut reg);
        let payload_start = self.base + WORD_SZ;

        if payload_start > self.pkt.len() as u32 {
            return Err(MarshalError::new(Errno::EBADMSG));
        }
        if nbyte as u64 + 1 > (self.pkt.len() as u32 - payload_start) as u64 {
            return Err(MarshalError::new(Errno::EBADMSG));
        }
        if self.pkt[(payload_start + nbyte) as usize] != 0 {
            return Err(MarshalError::new(Errno::EBADMSG));
        }

        Ok(String::from_utf8_lossy(&self.pkt[payload_start as usize..(payload_start + nbyte) as usize]).into_owned())
    }
}

#[derive(Clone, Debug)]
pub struct NullCursor<'a>(pub NodeCursor<'a>);

impl<'a> NullCursor<'a> {
    pub fn new(pkt: &'a [u8], type_: NodeType) -> NullCursor<'a> {
        NullCursor(NodeCursor::new(pkt, NONE, type_))
    }

    pub fn is_branch(&self) -> bool {
        false
    }

    pub fn as_branch(self) -> Result<BranchCursor<'a>, MarshalError> {
        Err(MarshalError::new(Errno::EFAULT))
    }

    pub fn opt(self) -> Option<NullCursor<'a>> {
        None
    }

    pub fn get(&self) -> ShallowNodeValue<'a> {
        NodeValue::Null
    }
}

#[derive(Clone, Debug)]
pub struct BranchCursor<'a> {
    pub cursor: NodeCursor<'a>,

    pub len: u32,
    pub child_start: u32, // first byte where child payloads may begin
}

impl PartialEq for BranchCursor<'_> {
    fn eq(&self, other: &BranchCursor<'_>) -> bool {
        core::ptr::eq(self.cursor.pkt, other.cursor.pkt) && self.cursor.base == other.cursor.base
    }
}

impl<'a> BranchCursor<'a> {
    pub fn new(pkt: &'a [u8], base: u32) -> Result<BranchCursor<'a>, MarshalError> {
        let cursor = NodeCursor::new(pkt, base, SpecialType::BRANCH as u8);
        let pkt = &cursor.pkt;

        if base + WORD_SZ > pkt.len() as u32 {
            return Err(MarshalError::new(Errno::EBADMSG));
        }
        let mut reg = WORD_REG;
        let [len, _] = GET_WORD(pkt, base, &mut reg);

        let table_off = base + WORD_SZ;
        if len > (pkt.len() as u32 - table_off) / WORD_SZ {
            return Err(MarshalError::new(Errno::EBADMSG));
        }

        let child_start = ENTRY_OFF(base, len);
        Ok(BranchCursor { cursor, len, child_start })
    }

    pub fn base(&self) -> u32 {
        self.cursor.base
    }

    pub fn is_branch(&self) -> bool {
        true
    }

    pub fn as_branch(self) -> Result<BranchCursor<'a>, MarshalError> {
        Ok(self)
    }

    pub fn opt(self) -> Option<BranchCursor<'a>> {
        Some(self)
    }

    /// A child branch comes back as its cursor, not recursed into.
    pub fn get(&self) -> Result<ShallowNodeValue<'a>, MarshalError> {
        let mut arr = Vec::with_capacity(self.len as usize);
        for i in 0..self.len {
            let cur = self.at(i)?;
            arr.push(match cur {
                Cursor::Branch(b) => NodeValue::Extra(b),
                c => c.get()?,
            });
        }
        Ok(NodeValue::List(arr))
    }

    pub fn get_at(&self, idx: u32) -> Result<ShallowNodeValue<'a>, MarshalError> {
        self.at(idx)?.get()
    }

    pub fn get_deep(&self) -> Result<NodeValue<'a>, MarshalError> {
        let mut arr = Vec::with_capacity(self.len as usize);
        for i in 0..self.len {
            let cur = self.at(i)?;
            arr.push(cur.get_deep()?);
        }
        Ok(NodeValue::List(arr))
    }

    /// A zero offset is an absent child, which keeps its type.
    pub fn at(&self, idx: u32) -> Result<Cursor<'a>, MarshalError> {
        if idx >= self.len {
            return Err(MarshalError::new(Errno::EFAULT));
        }
        let entry_off = ENTRY_OFF(self.cursor.base, idx);

        let mut reg = WORD_REG;
        let [rel_off, desc] = GET_WORD(self.cursor.pkt, entry_off, &mut reg);
        let type_: NodeType = field_get32(desc, DESC_TYPE_SHAMT, DESC_TYPE_MASK) as NodeType;

        if rel_off == 0 {
            return Ok(Cursor::Null(NullCursor::new(self.cursor.pkt, type_)));
        }
        if rel_off > self.cursor.pkt.len() as u32 - self.cursor.base {
            return Err(MarshalError::new(Errno::EBADMSG));
        }
        let base = self.cursor.base + rel_off;
        if base < self.child_start || base >= self.cursor.pkt.len() as u32 {
            return Err(MarshalError::new(Errno::EBADMSG));
        }
        CURSOR(self.cursor.pkt, base, type_)
    }

    pub fn at_branch(&self, idx: u32) -> Result<BranchCursor<'a>, MarshalError> {
        self.at(idx)?.as_branch()
    }

    pub fn children(&self) -> Result<Vec<Cursor<'a>>, MarshalError> {
        (0..self.len).map(|i| self.at(i)).collect()
    }

    pub fn iter(&self) -> impl Iterator<Item = Result<Cursor<'a>, MarshalError>> + '_ {
        (0..self.len).map(move |i| self.at(i))
    }
}

fn bytes_of<T: Copy>(a: &[T]) -> &[u8] {
    // SAFETY: the elements are the packet's own bytes, read back as bytes.
    unsafe { core::slice::from_raw_parts(a.as_ptr() as *const u8, core::mem::size_of_val(a)) }
}

pub fn elements<'b, T: Copy>(a: &'b Array<'_>) -> &'b [T] {
    let n = a.bytes.len() / core::mem::size_of::<T>();
    // SAFETY: a leaf's bytes are its elements, as the packet laid them out.
    unsafe { core::slice::from_raw_parts(a.bytes.as_ptr() as *const T, n) }
}

#[cfg(test)]
#[path = "cursor.test.rs"]
mod test;

/// Validated as UTF-8 when the sequence was read.
#[derive(Clone, Copy, Debug)]
pub struct Strs<'a> {
    offs: &'a [u8],
    blob: &'a [u8],
    len: u32,
}

impl<'a> Strs<'a> {
    fn off(&self, i: usize) -> usize {
        u32::from_le_bytes([self.offs[4 * i], self.offs[4 * i + 1], self.offs[4 * i + 2], self.offs[4 * i + 3]]) as usize
    }

    pub fn len(&self) -> u32 {
        self.len
    }

    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    pub fn get(&self, i: u32) -> Option<&'a str> {
        (i < self.len).then(|| {
            let blob: &'a [u8] = self.blob;
            // SAFETY: every span was checked as UTF-8 when the sequence was read.
            unsafe { core::str::from_utf8_unchecked(&blob[self.off(i as usize)..self.off(i as usize + 1)]) }
        })
    }

    pub fn iter(&self) -> impl Iterator<Item = &'a str> + 'a {
        let s = *self;
        (0..s.len).filter_map(move |i| s.get(i))
    }
}

pub trait ArrayElem: Copy {
    const TYPE: SequenceType;
}

macro_rules! array_elem {
    ($($t:ty => $seq:ident),* $(,)?) => {
        $(impl ArrayElem for $t {
            const TYPE: SequenceType = SequenceType::$seq;
        })*
    };
}

array_elem!(u8 => U8_ARRAY, i8 => I8_ARRAY, u16 => U16_ARRAY, i16 => I16_ARRAY, u32 => U32_ARRAY, i32 => I32_ARRAY, u64 => U64_ARRAY, i64 => I64_ARRAY, f32 => F32_ARRAY, f64 => F64_ARRAY);
