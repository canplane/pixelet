// xpute-core/wire/xtp/cursor.rs

//! The read end: lazy cursors over a packet. A read hands back a `Cursor`,
//! which is one of three — a leaf's `NodeCursor`, a `NullCursor` for a node
//! that is physically absent but keeps its type, or a `BranchCursor` over a
//! child table — since a child can be any of them. Nothing is decoded until
//! a `get`.

// Uppercase throughout, as the notation these operations are written in: one
// that holds no state and closes over nothing is named the way a C header
// names its macros. Rust's own answer to a macro like that is a `const fn`,
// which it cases snake, so the notation and the language disagree here and
// the file keeps the notation — suspended once for the file, which is the
// level the choice is made at rather than item by item.
#![allow(non_snake_case)]

use crate::abi::word::field_get32;
use crate::status::errno::Errno;
use crate::status::error::MarshalError;

use super::spec::{
    AlignUnit, Array, NodeType, NodeValue, ScalarType, SequenceType, SpecialType, ALIGN_SZ, DESC_TYPE_MASK, DESC_TYPE_SHAMT, GET_WORD, HDR_SZ, MAGIC, NONE, TYPE_IS_LEAF, TYPE_IS_SEQ, WORD_SZ,
};

/// A shallow read's value: a leaf's value, null, or a branch's cursor.
pub type ShallowNodeValue<'a> = NodeValue<'a, BranchCursor<'a>>;

/// Scratchpad for 64-bit word operations to avoid allocation.
/// [lo32, hi32]
const WORD_REG: [u32; 2] = [NONE, NONE];

fn CURSOR(pkt: &[u8], base: u32, type_: NodeType) -> Result<Cursor<'_>, MarshalError> {
    let align_sz: AlignUnit = ALIGN_SZ(type_)?;
    if !base.is_multiple_of(align_sz) {
        return Err(MarshalError::new(Errno::EBADMSG, Some(&format!("misaligned node base {base} for type 0x{type_:x}")), None));
    }
    if type_ == SpecialType::BRANCH as u8 {
        return Ok(Cursor::Branch(BranchCursor::new(pkt, base)?));
    }
    if TYPE_IS_LEAF(type_) {
        return Ok(Cursor::Node(NodeCursor::new(pkt, base, type_)));
    }
    Err(MarshalError::new(Errno::EBADMSG, Some(&format!("unsupported special node type: 0x{type_:x}")), None))
}

fn ENTRY_OFF(base: u32, idx: u32) -> u32 {
    (base + WORD_SZ) + idx * WORD_SZ
}

/// What a read hands back: a leaf cursor, a null cursor or a branch cursor.
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

    /// Reinterpret current node as a branch cursor.
    /// Fails if the current node is not a branch.
    pub fn as_branch(self) -> Result<BranchCursor<'a>, MarshalError> {
        match self {
            Cursor::Branch(b) => Ok(b),
            Cursor::Null(n) => n.as_branch(),
            Cursor::Node(c) => c.as_branch(),
        }
    }

    /// Optional chaining helper: `None` for a physically absent node.
    pub fn opt(self) -> Option<Cursor<'a>> {
        match self {
            Cursor::Null(_) => None,
            c => Some(c),
        }
    }

    /// Reads the current node at shallow depth.
    pub fn get(&self) -> Result<ShallowNodeValue<'a>, MarshalError> {
        match self {
            Cursor::Node(c) => c.get(),
            Cursor::Null(c) => Ok(c.get()),
            Cursor::Branch(c) => c.get(),
        }
    }

    /// A typed-array leaf's elements, as the type the caller names. A read
    /// of any other node is EBADMSG rather than a wrong slice.
    pub fn get_array<T: Copy>(&self) -> Result<&'a [T], MarshalError> {
        match self {
            Cursor::Node(c) if TYPE_IS_SEQ(c.type_) && c.type_ != SequenceType::BITSET as u8 && c.type_ != SequenceType::STR as u8 => c._array::<T>(),
            _ => Err(MarshalError::new(Errno::EBADMSG, Some(&format!("expected an array, got type 0x{:x}", self.type_())), None)),
        }
    }

    /// `get<u64>()`, checked as `get_array` is.
    pub fn get_u64(&self) -> Result<u64, MarshalError> {
        match self.get()? {
            NodeValue::U64(x) => Ok(x),
            _ => Err(MarshalError::new(Errno::EBADMSG, Some(&format!("expected a u64, got type 0x{:x}", self.type_())), None)),
        }
    }

    /// `get<i64>()`, checked as `get_array` is.
    pub fn get_i64(&self) -> Result<i64, MarshalError> {
        match self.get()? {
            NodeValue::I64(x) => Ok(x),
            _ => Err(MarshalError::new(Errno::EBADMSG, Some(&format!("expected an i64, got type 0x{:x}", self.type_())), None)),
        }
    }

    /// `get<boolean>()`, checked as `get_array` is.
    pub fn get_bool(&self) -> Result<bool, MarshalError> {
        match self.get()? {
            NodeValue::Bool(b) => Ok(b),
            _ => Err(MarshalError::new(Errno::EBADMSG, Some(&format!("expected a boolean, got type 0x{:x}", self.type_())), None)),
        }
    }

    /// `get<string>()`, checked as `get_array` is.
    pub fn get_str(&self) -> Result<String, MarshalError> {
        match self.get()? {
            NodeValue::Str(s) => Ok(s),
            _ => Err(MarshalError::new(Errno::EBADMSG, Some(&format!("expected a string, got type 0x{:x}", self.type_())), None)),
        }
    }

    /// `get<string>()` read where it lies: the packet's own bytes, with
    /// nothing decoded into a string of its own — it outlives the cursor and
    /// lives as long as the packet does. A string that is not UTF-8 is
    /// EBADMSG, where the host's decode would have replaced what does not
    /// read.
    pub fn get_str_ref(&self) -> Result<&'a str, MarshalError> {
        match self {
            Cursor::Node(c) if c.type_ == SequenceType::STR as u8 => c._str_ref(),
            _ => Err(MarshalError::new(Errno::EBADMSG, Some(&format!("expected a string, got type 0x{:x}", self.type_())), None)),
        }
    }

    /// A scalar leaf of any width up to 32 bits, or a float, as a double —
    /// every one of those is exact in an f64. A 64-bit integer is not, and is
    /// not here: `get_u64`/`get_i64`.
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
            _ => return Err(MarshalError::new(Errno::EBADMSG, Some(&format!("expected a number, got type 0x{:x}", self.type_())), None)),
        })
    }

    /// Reads the current node at full depth.
    pub fn get_deep(&self) -> Result<NodeValue<'a>, MarshalError> {
        match self {
            Cursor::Node(c) => Ok(deepen(c.get()?)),
            Cursor::Null(c) => Ok(deepen(c.get())),
            Cursor::Branch(c) => c.get_deep(),
        }
    }
}

/// A leaf's or null's shallow value holds no cursor, so it is its deep value.
fn deepen(v: ShallowNodeValue<'_>) -> NodeValue<'_> {
    match v {
        NodeValue::Extra(_) => unreachable!("a leaf's shallow value holds no cursor"),
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

// ============ Reader ============

/// Packet reader and root cursor entry point.
///
/// Contract:
/// - requires packet base offset to be WORD_SZ-aligned
/// - validates the fixed packet header
/// - returns a root cursor on success
/// - errs on malformed packet
pub struct TreeReader<'a> {
    pub pkt: &'a [u8],

    pub root: Cursor<'a>,
}

impl<'a> TreeReader<'a> {
    pub fn new(pkt: &'a [u8]) -> Result<TreeReader<'a>, MarshalError> {
        let base = pkt.as_ptr() as usize;
        if !base.is_multiple_of(WORD_SZ as usize) {
            return Err(MarshalError::new(Errno::EBADMSG, Some(&format!("bad packet: misaligned base offset {base}")), None));
        }

        if (pkt.len() as u32) < HDR_SZ {
            return Err(MarshalError::new(Errno::EBADMSG, Some("bad packet: truncated header"), None));
        }
        let hdr_view = &pkt[..HDR_SZ as usize];

        // packet header
        let mut reg = WORD_REG;
        let [magic, _] = GET_WORD(hdr_view, 0, &mut reg); // word0
        if magic != MAGIC {
            return Err(MarshalError::new(Errno::EBADMSG, Some("bad packet: magic mismatch"), None));
        }

        // word1: the root header stores payload byte size, not a relative offset
        let [payload_sz, desc] = GET_WORD(hdr_view, WORD_SZ, &mut reg);
        if payload_sz > pkt.len() as u32 - HDR_SZ {
            return Err(MarshalError::new(Errno::EBADMSG, Some("bad packet: truncated payload"), None));
        }
        let type_: NodeType = field_get32(desc, DESC_TYPE_SHAMT, DESC_TYPE_MASK) as NodeType;

        let root = if payload_sz != 0 {
            CURSOR(pkt, HDR_SZ, type_)?
        } else {
            Cursor::Null(NullCursor::new(pkt, type_))
        };
        Ok(TreeReader { pkt, root })
    }

    /// Returns the root cursor after constructor-time packet validation.
    pub fn read(&self) -> Cursor<'a> {
        self.root.clone()
    }

    pub fn read_branch(&self) -> Result<BranchCursor<'a>, MarshalError> {
        self.root.clone().as_branch()
    }
}

// ============ Cursors ============

#[derive(Clone, Debug)]
pub struct NodeCursor<'a> {
    pub pkt: &'a [u8],
    pub base: u32,

    pub type_: NodeType,
}

impl<'a> NodeCursor<'a> {
    /// - `pkt`: source packet bytes
    /// - `base`: logical node base offset
    pub fn new(pkt: &'a [u8], base: u32, type_: NodeType) -> NodeCursor<'a> {
        NodeCursor { pkt, base, type_ }
    }

    pub fn is_branch(&self) -> bool {
        self.type_ == SpecialType::BRANCH as u8
    }

    /// Reinterpret current node as a branch cursor.
    /// Fails if the current node is not a branch.
    pub fn as_branch(self) -> Result<BranchCursor<'a>, MarshalError> {
        if self.type_ != SpecialType::BRANCH as u8 {
            return Err(MarshalError::new(Errno::EBADMSG, Some(&format!("node is not a branch: 0x{:x}", self.type_)), None));
        }
        BranchCursor::new(self.pkt, self.base)
    }

    // ---- Optional Guard ----

    /// Optional chaining helper.
    /// Returns `this` for a present node.
    /// NullCursor's returns `None`.
    pub fn opt(self) -> Option<NodeCursor<'a>> {
        Some(self)
    }

    // ---- Shallow Getter ----

    /// Reads the current node at shallow depth.
    ///
    /// Shallow contract:
    /// - leaf cursor  -> materialized leaf value
    /// - null cursor  -> null
    /// - branch cursor is handled by BranchCursor's
    pub fn get(&self) -> Result<ShallowNodeValue<'a>, MarshalError> {
        let t = self.type_;
        Ok(match t {
            // -- Scalars --
            t if t == ScalarType::U8 as u8 => NodeValue::U8(self._u8()?),
            t if t == ScalarType::I8 as u8 => NodeValue::I8(self._i8()?),
            t if t == ScalarType::U16 as u8 => NodeValue::U16(self._u16()?),
            t if t == ScalarType::I16 as u8 => NodeValue::I16(self._i16()?),
            t if t == ScalarType::U32 as u8 => NodeValue::U32(self._u32()?),
            t if t == ScalarType::I32 as u8 => NodeValue::I32(self._i32()?),
            t if t == ScalarType::U64 as u8 => NodeValue::U64(self._u64()?),
            t if t == ScalarType::I64 as u8 => NodeValue::I64(self._i64()?),
            t if t == ScalarType::F32 as u8 => NodeValue::F32(self._f32()?),
            t if t == ScalarType::F64 as u8 => NodeValue::F64(self._f64()?),
            t if t == ScalarType::BOOL as u8 => NodeValue::Bool(self._bool()?),

            // -- Sequences --
            t if t == SequenceType::U8_ARRAY as u8 => NodeValue::Array(Array {
                type_: SequenceType::U8_ARRAY,
                bytes: bytes_of(self._u8_array()?).into(),
            }),
            t if t == SequenceType::I8_ARRAY as u8 => NodeValue::Array(Array {
                type_: SequenceType::I8_ARRAY,
                bytes: bytes_of(self._i8_array()?).into(),
            }),
            t if t == SequenceType::U16_ARRAY as u8 => NodeValue::Array(Array {
                type_: SequenceType::U16_ARRAY,
                bytes: bytes_of(self._u16_array()?).into(),
            }),
            t if t == SequenceType::I16_ARRAY as u8 => NodeValue::Array(Array {
                type_: SequenceType::I16_ARRAY,
                bytes: bytes_of(self._i16_array()?).into(),
            }),
            t if t == SequenceType::U32_ARRAY as u8 => NodeValue::Array(Array {
                type_: SequenceType::U32_ARRAY,
                bytes: bytes_of(self._u32_array()?).into(),
            }),
            t if t == SequenceType::I32_ARRAY as u8 => NodeValue::Array(Array {
                type_: SequenceType::I32_ARRAY,
                bytes: bytes_of(self._i32_array()?).into(),
            }),
            t if t == SequenceType::U64_ARRAY as u8 => NodeValue::Array(Array {
                type_: SequenceType::U64_ARRAY,
                bytes: bytes_of(self._u64_array()?).into(),
            }),
            t if t == SequenceType::I64_ARRAY as u8 => NodeValue::Array(Array {
                type_: SequenceType::I64_ARRAY,
                bytes: bytes_of(self._i64_array()?).into(),
            }),
            t if t == SequenceType::F32_ARRAY as u8 => NodeValue::Array(Array {
                type_: SequenceType::F32_ARRAY,
                bytes: bytes_of(self._f32_array()?).into(),
            }),
            t if t == SequenceType::F64_ARRAY as u8 => NodeValue::Array(Array {
                type_: SequenceType::F64_ARRAY,
                bytes: bytes_of(self._f64_array()?).into(),
            }),
            t if t == SequenceType::BITSET as u8 => NodeValue::Array(Array {
                type_: SequenceType::BITSET,
                bytes: self._bitset()?.into(),
            }),
            t if t == SequenceType::STR as u8 => NodeValue::Str(self._str()?),

            _ => return Err(MarshalError::new(Errno::EBADMSG, Some(&format!("unknown node type 0x{t:x}")), None)),
        })
    }

    // ---- Internal Leaf Readers ----

    fn scalar<const N: usize>(&self) -> Result<[u8; N], MarshalError> {
        if self.base as usize + N > self.pkt.len() {
            return Err(MarshalError::new(Errno::EBADMSG, Some("scalar OOB"), None));
        }
        Ok(self.pkt[self.base as usize..self.base as usize + N].try_into().unwrap())
    }

    fn _u8(&self) -> Result<u8, MarshalError> {
        Ok(u8::from_le_bytes(self.scalar()?))
    }
    fn _i8(&self) -> Result<i8, MarshalError> {
        Ok(i8::from_le_bytes(self.scalar()?))
    }
    fn _u16(&self) -> Result<u16, MarshalError> {
        Ok(u16::from_le_bytes(self.scalar()?))
    }
    fn _i16(&self) -> Result<i16, MarshalError> {
        Ok(i16::from_le_bytes(self.scalar()?))
    }
    fn _u32(&self) -> Result<u32, MarshalError> {
        Ok(u32::from_le_bytes(self.scalar()?))
    }
    fn _i32(&self) -> Result<i32, MarshalError> {
        Ok(i32::from_le_bytes(self.scalar()?))
    }
    fn _u64(&self) -> Result<u64, MarshalError> {
        Ok(u64::from_le_bytes(self.scalar()?))
    }
    fn _i64(&self) -> Result<i64, MarshalError> {
        Ok(i64::from_le_bytes(self.scalar()?))
    }
    fn _f32(&self) -> Result<f32, MarshalError> {
        Ok(f32::from_le_bytes(self.scalar()?))
    }
    fn _f64(&self) -> Result<f64, MarshalError> {
        Ok(f64::from_le_bytes(self.scalar()?))
    }
    fn _bool(&self) -> Result<bool, MarshalError> {
        Ok(u8::from_le_bytes(self.scalar()?) != 0)
    }

    /// Reads a typed-array sequence leaf as a zero-copy view over the packet buffer.
    ///
    /// Contract:
    /// - checks only len/payload overflow against packet bounds
    /// - assumes packet base alignment was validated by TreeReader
    /// - sequence payload starts immediately after the len slot: base + WORD_SZ
    /// - because sequence node bases are WORD_SZ-aligned, payload_start is also
    ///   naturally aligned for all supported typed-array element sizes (<= 8)
    fn _array<T: Copy>(&self) -> Result<&'a [T], MarshalError> {
        if self.base + WORD_SZ > self.pkt.len() as u32 {
            return Err(MarshalError::new(Errno::EBADMSG, Some("len slot OOB"), None));
        }
        let mut reg = WORD_REG;
        let [len, _] = GET_WORD(self.pkt, self.base, &mut reg);
        let payload_start = self.base + WORD_SZ;

        let elem_sz = core::mem::size_of::<T>() as u32;
        let payload_sz = len.wrapping_mul(elem_sz);

        if len != 0 && payload_sz / elem_sz != len {
            return Err(MarshalError::new(Errno::EBADMSG, Some("payload size overflow"), None));
        }

        if payload_start > self.pkt.len() as u32 {
            return Err(MarshalError::new(Errno::EBADMSG, Some("payload OOB"), None));
        }
        if payload_sz > self.pkt.len() as u32 - payload_start {
            return Err(MarshalError::new(Errno::EBADMSG, Some("payload OOB"), None));
        }

        let at = payload_start as usize;
        // SAFETY: a sequence node's base is WORD_SZ-aligned and the packet's
        // own base was checked, so the payload is aligned for every element
        // the format carries (8 bytes at most); the bounds above hold it
        // inside the packet.
        Ok(unsafe { core::slice::from_raw_parts(self.pkt[at..].as_ptr() as *const T, len as usize) })
    }

    fn _u8_array(&self) -> Result<&'a [u8], MarshalError> {
        self._array()
    }
    fn _i8_array(&self) -> Result<&'a [i8], MarshalError> {
        self._array()
    }
    fn _u16_array(&self) -> Result<&'a [u16], MarshalError> {
        self._array()
    }
    fn _i16_array(&self) -> Result<&'a [i16], MarshalError> {
        self._array()
    }
    fn _u32_array(&self) -> Result<&'a [u32], MarshalError> {
        self._array()
    }
    fn _i32_array(&self) -> Result<&'a [i32], MarshalError> {
        self._array()
    }
    fn _u64_array(&self) -> Result<&'a [u64], MarshalError> {
        self._array()
    }
    fn _i64_array(&self) -> Result<&'a [i64], MarshalError> {
        self._array()
    }
    fn _f32_array(&self) -> Result<&'a [f32], MarshalError> {
        self._array()
    }
    fn _f64_array(&self) -> Result<&'a [f64], MarshalError> {
        self._array()
    }

    fn _bitset(&self) -> Result<Vec<u8>, MarshalError> {
        if self.base + WORD_SZ > self.pkt.len() as u32 {
            return Err(MarshalError::new(Errno::EBADMSG, Some("len slot OOB"), None));
        }
        let mut reg = WORD_REG;
        let [len, _] = GET_WORD(self.pkt, self.base, &mut reg);
        let payload_start = self.base + WORD_SZ;

        // A wire length: u64, so a count near 2^32 cannot wrap to a small size.
        let payload_sz = (len as u64 + 7) >> 3;
        if payload_start > self.pkt.len() as u32 {
            return Err(MarshalError::new(Errno::EBADMSG, Some("payload OOB"), None));
        }
        if payload_sz > (self.pkt.len() as u32 - payload_start) as u64 {
            return Err(MarshalError::new(Errno::EBADMSG, Some("payload OOB"), None));
        }

        let mut arr = vec![0u8; len as usize];
        for (i, slot) in arr.iter_mut().enumerate() {
            let x = self.pkt[payload_start as usize + (i >> 3)] & (1 << (i & 7));
            *slot = (x != 0) as u8;
        }
        Ok(arr)
    }

    /// The string's own bytes where they lie, checked as `_str` checks them.
    fn _str_ref(&self) -> Result<&'a str, MarshalError> {
        if self.base + WORD_SZ > self.pkt.len() as u32 {
            return Err(MarshalError::new(Errno::EBADMSG, Some("len slot OOB"), None));
        }
        let mut reg = WORD_REG;
        let [nbyte, _] = GET_WORD(self.pkt, self.base, &mut reg);
        let payload_start = self.base + WORD_SZ;
        if payload_start > self.pkt.len() as u32 || nbyte as u64 + 1 > (self.pkt.len() as u32 - payload_start) as u64 {
            return Err(MarshalError::new(Errno::EBADMSG, Some("payload OOB"), None));
        }
        let bytes = self.pkt;
        if bytes[(payload_start + nbyte) as usize] != 0 {
            return Err(MarshalError::new(Errno::EBADMSG, Some("missing string terminator"), None));
        }
        core::str::from_utf8(&bytes[payload_start as usize..(payload_start + nbyte) as usize]).map_err(|_| MarshalError::new(Errno::EBADMSG, Some("a string that is not UTF-8"), None))
    }

    // STR stores UTF-8 bytes with a trailing NUL on wire, while len excludes that terminator.
    fn _str(&self) -> Result<String, MarshalError> {
        if self.base + WORD_SZ > self.pkt.len() as u32 {
            return Err(MarshalError::new(Errno::EBADMSG, Some("len slot OOB"), None));
        }
        let mut reg = WORD_REG;
        let [nbyte, _] = GET_WORD(self.pkt, self.base, &mut reg);
        let payload_start = self.base + WORD_SZ;

        if payload_start > self.pkt.len() as u32 {
            return Err(MarshalError::new(Errno::EBADMSG, Some("payload OOB"), None));
        }
        if nbyte as u64 + 1 > (self.pkt.len() as u32 - payload_start) as u64 {
            return Err(MarshalError::new(Errno::EBADMSG, Some("payload OOB"), None));
        }
        if self.pkt[(payload_start + nbyte) as usize] != 0 {
            return Err(MarshalError::new(Errno::EBADMSG, Some("missing string terminator"), None));
        }

        Ok(String::from_utf8_lossy(&self.pkt[payload_start as usize..(payload_start + nbyte) as usize]).into_owned())
    }
}

#[derive(Clone, Debug)]
pub struct NullCursor<'a>(pub NodeCursor<'a>);

impl<'a> NullCursor<'a> {
    /// - `pkt`: source packet bytes
    pub fn new(pkt: &'a [u8], type_: NodeType) -> NullCursor<'a> {
        NullCursor(NodeCursor::new(pkt, NONE, type_))
    }

    // NullCursor preserves original node type metadata,
    // but is never considered a usable structural cursor.
    pub fn is_branch(&self) -> bool {
        false
    }

    /// Reinterpret current node as a branch cursor.
    /// Fails if the current node is not a branch.
    pub fn as_branch(self) -> Result<BranchCursor<'a>, MarshalError> {
        Err(MarshalError::new(Errno::EFAULT, Some("cannot cast a null node to a branch"), None))
    }

    // ---- Nullable Guard ----

    /// Optional chaining helper.
    /// Returns `None` for a physically absent node and `this` otherwise.
    pub fn opt(self) -> Option<NullCursor<'a>> {
        None
    }

    // ---- Shallow Getter ----

    /// Shallow read of a physically absent node reconstructs generic null.
    pub fn get(&self) -> ShallowNodeValue<'a> {
        // generic optional reconstruction
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
            return Err(MarshalError::new(Errno::EBADMSG, Some("branch header OOB"), None));
        }
        let mut reg = WORD_REG;
        let [len, _] = GET_WORD(pkt, base, &mut reg);

        let table_off = base + WORD_SZ;
        if len > (pkt.len() as u32 - table_off) / WORD_SZ {
            return Err(MarshalError::new(Errno::EBADMSG, Some("branch table overflow"), None));
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

    // ---- Shallow / Deep Getter ----

    /// Shallow branch read.
    ///
    /// Contract:
    /// - get()      -> array of direct children
    /// - leaf child -> materialized leaf value
    /// - branch child -> its cursor (not recursive)
    /// - null child -> null
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

    /// get(idx): the direct child at idx, shallow-materialized at that
    /// child boundary.
    pub fn get_at(&self, idx: u32) -> Result<ShallowNodeValue<'a>, MarshalError> {
        self.at(idx)?.get()
    }

    /// Deep branch read.
    ///
    /// Recursively materializes the entire child subtree.
    pub fn get_deep(&self) -> Result<NodeValue<'a>, MarshalError> {
        let mut arr = Vec::with_capacity(self.len as usize);
        for i in 0..self.len {
            let cur = self.at(i)?;
            arr.push(cur.get_deep()?);
        }
        Ok(NodeValue::List(arr))
    }

    /// Returns the child cursor at `idx`.
    ///
    /// Contract:
    /// - preserves child type metadata even when physically absent
    /// - returns NullCursor when child_rel_off == 0
    /// - validates child offset bounds against the enclosing branch table
    pub fn at(&self, idx: u32) -> Result<Cursor<'a>, MarshalError> {
        if idx >= self.len {
            return Err(MarshalError::new(Errno::EFAULT, Some(&format!("child index out of bounds: {idx}")), None));
        }
        let entry_off = ENTRY_OFF(self.cursor.base, idx);

        let mut reg = WORD_REG;
        let [rel_off, desc] = GET_WORD(self.cursor.pkt, entry_off, &mut reg);
        let type_: NodeType = field_get32(desc, DESC_TYPE_SHAMT, DESC_TYPE_MASK) as NodeType;

        if rel_off == 0 {
            return Ok(Cursor::Null(NullCursor::new(self.cursor.pkt, type_)));
        }
        if rel_off > self.cursor.pkt.len() as u32 - self.cursor.base {
            return Err(MarshalError::new(Errno::EBADMSG, Some("child offset overflow"), None));
        }
        let base = self.cursor.base + rel_off;
        if base < self.child_start || base >= self.cursor.pkt.len() as u32 {
            return Err(MarshalError::new(Errno::EBADMSG, Some(&format!("bad child offset: base={} rel_off={rel_off}", self.cursor.base)), None));
        }
        CURSOR(self.cursor.pkt, base, type_)
    }

    pub fn at_branch(&self, idx: u32) -> Result<BranchCursor<'a>, MarshalError> {
        self.at(idx)?.as_branch()
    }

    /// Returns all direct child cursors without materializing them.
    pub fn children(&self) -> Result<Vec<Cursor<'a>>, MarshalError> {
        (0..self.len).map(|i| self.at(i)).collect()
    }

    /// The children in order.
    pub fn iter(&self) -> impl Iterator<Item = Result<Cursor<'a>, MarshalError>> + '_ {
        (0..self.len).map(move |i| self.at(i))
    }
}

/// A leaf's elements as the bytes they lie in.
fn bytes_of<T: Copy>(a: &[T]) -> &[u8] {
    // SAFETY: the elements are the packet's own bytes, read back as bytes.
    unsafe { core::slice::from_raw_parts(a.as_ptr() as *const u8, core::mem::size_of_val(a)) }
}

/// A sequence leaf's bytes read as the element the caller names.
pub fn elements<'b, T: Copy>(a: &'b Array<'_>) -> &'b [T] {
    let n = a.bytes.len() / core::mem::size_of::<T>();
    // SAFETY: a leaf's bytes are its elements, as the packet laid them out.
    unsafe { core::slice::from_raw_parts(a.bytes.as_ptr() as *const T, n) }
}

#[cfg(test)]
#[path = "cursor.test.rs"]
mod test;
