// xpute-core/io/marshal/tree/cursor.rs

//! The read end: lazy cursors over a packet. The TypeScript's class
//! hierarchy — `NodeCursor`, with `NullCursor` and `BranchCursor` extending
//! it — is three structs and the `Cursor` enum a read hands back, since a
//! child can be any of the three.

#![allow(non_snake_case)]

use crate::kit::array::{AnyElement, ArrayBufferView, Bytes, TypedArray, U8Array};
use crate::kit::encoding;
use crate::sys::errno::Errno;
use crate::sys::error::MarshalError;

use super::spec::{AlignUnit, NodeType, NodeValue, ScalarType, SequenceType, SpecialType, ALIGN_SZ, DESC_TYPE_MASK, DESC_TYPE_SHAMT, GET_WORD, HDR_SZ, MAGIC, NONE, TYPE_IS_LEAF, WORD_SZ};

/// A shallow read's value: a leaf's value, null, or a branch's cursor.
pub type ShallowNodeValue = NodeValue<BranchCursor>;

/// Scratchpad for 64-bit word operations to avoid allocation.
/// [lo32, hi32]
const WORD_REG: [u32; 2] = [NONE, NONE];

fn field_get(u: u32, shamt: u32, mask: u32) -> u32 {
    (u >> shamt) & mask
}

fn CURSOR(pkt: &U8Array, base: u32, type_: NodeType) -> Result<Cursor, MarshalError> {
    let align_sz: AlignUnit = ALIGN_SZ(type_)?;
    if !base.is_multiple_of(align_sz) {
        return Err(MarshalError::new(Errno::EBADMSG, Some(&format!("misaligned node base {base} for type 0x{type_:x}")), None));
    }
    if type_ == SpecialType::BRANCH as u8 {
        return Ok(Cursor::Branch(BranchCursor::new(pkt.clone(), base)?));
    }
    if TYPE_IS_LEAF(type_) {
        return Ok(Cursor::Node(NodeCursor::new(pkt.clone(), base, type_)));
    }
    Err(MarshalError::new(Errno::EBADMSG, Some(&format!("unsupported special node type: 0x{type_:x}")), None))
}

fn ENTRY_OFF(base: u32, idx: u32) -> u32 {
    (base + WORD_SZ) + idx * WORD_SZ
}

/// What a read hands back: a leaf cursor, a null cursor or a branch cursor.
#[derive(Clone, Debug)]
pub enum Cursor {
    Node(NodeCursor),
    Null(NullCursor),
    Branch(BranchCursor),
}

impl Cursor {
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
    pub fn as_branch(self) -> Result<BranchCursor, MarshalError> {
        match self {
            Cursor::Branch(b) => Ok(b),
            Cursor::Null(n) => n.as_branch(),
            Cursor::Node(c) => c.as_branch(),
        }
    }

    /// Optional chaining helper: `None` for a physically absent node.
    pub fn opt(self) -> Option<Cursor> {
        match self {
            Cursor::Null(_) => None,
            c => Some(c),
        }
    }

    /// Reads the current node at shallow depth.
    pub fn get(&self) -> Result<ShallowNodeValue, MarshalError> {
        match self {
            Cursor::Node(c) => c.get(),
            Cursor::Null(c) => Ok(c.get()),
            Cursor::Branch(c) => c.get(),
        }
    }

    /// Reads the current node at full depth.
    pub fn get_deep(&self) -> Result<NodeValue, MarshalError> {
        match self {
            Cursor::Node(c) => Ok(deepen(c.get()?)),
            Cursor::Null(c) => Ok(deepen(c.get())),
            Cursor::Branch(c) => c.get_deep(),
        }
    }
}

/// A leaf's or null's shallow value holds no cursor, so it is its deep value.
fn deepen(v: ShallowNodeValue) -> NodeValue {
    match v {
        NodeValue::Extra(_) => unreachable!("a leaf's shallow value holds no cursor"),
        NodeValue::Null => NodeValue::Null,
        NodeValue::Bool(b) => NodeValue::Bool(b),
        NodeValue::Number(n) => NodeValue::Number(n),
        NodeValue::BigInt(n) => NodeValue::BigInt(n),
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
pub struct TreeReader {
    pub pkt: U8Array,

    pub root: Cursor,
}

impl TreeReader {
    pub fn new(pkt: U8Array) -> Result<TreeReader, MarshalError> {
        if !pkt.byte_offset().is_multiple_of(WORD_SZ as usize) {
            return Err(MarshalError::new(Errno::EBADMSG, Some(&format!("bad packet: misaligned base offset {}", pkt.byte_offset())), None));
        }

        let pkt = TypedArray::<u8>::over(pkt.buffer().clone(), pkt.byte_offset(), pkt.byte_length());

        if (pkt.byte_length() as u32) < HDR_SZ {
            return Err(MarshalError::new(Errno::EBADMSG, Some("bad packet: truncated header"), None));
        }
        let hdr_view: Bytes = TypedArray::over(pkt.buffer().clone(), pkt.byte_offset(), HDR_SZ as usize);

        // packet header
        let mut reg = WORD_REG;
        let [magic, _] = GET_WORD(&hdr_view, 0, &mut reg); // word0
        if magic != MAGIC {
            return Err(MarshalError::new(Errno::EBADMSG, Some("bad packet: magic mismatch"), None));
        }

        // word1: the root header stores payload byte size, not a relative offset
        let [payload_sz, desc] = GET_WORD(&hdr_view, WORD_SZ, &mut reg);
        if payload_sz > pkt.byte_length() as u32 - HDR_SZ {
            return Err(MarshalError::new(Errno::EBADMSG, Some("bad packet: truncated payload"), None));
        }
        let type_: NodeType = field_get(desc, DESC_TYPE_SHAMT, DESC_TYPE_MASK) as NodeType;

        let root = if payload_sz != 0 {
            CURSOR(&pkt, HDR_SZ, type_)?
        } else {
            Cursor::Null(NullCursor::new(pkt.clone(), type_))
        };
        Ok(TreeReader { pkt, root })
    }

    /// Returns the root cursor after constructor-time packet validation.
    pub fn read(&self) -> Cursor {
        self.root.clone()
    }

    pub fn read_branch(&self) -> Result<BranchCursor, MarshalError> {
        self.root.clone().as_branch()
    }
}

// ============ Cursors ============

#[derive(Clone, Debug)]
pub struct NodeCursor {
    pub pkt: U8Array,
    pub base: u32,

    pub type_: NodeType,
}

impl NodeCursor {
    /// - `pkt`: source packet bytes
    /// - `base`: logical node base offset
    pub fn new(pkt: U8Array, base: u32, type_: NodeType) -> NodeCursor {
        NodeCursor { pkt, base, type_ }
    }

    pub fn is_branch(&self) -> bool {
        self.type_ == SpecialType::BRANCH as u8
    }

    /// Reinterpret current node as a branch cursor.
    /// Fails if the current node is not a branch.
    pub fn as_branch(self) -> Result<BranchCursor, MarshalError> {
        if self.type_ != SpecialType::BRANCH as u8 {
            return Err(MarshalError::new(Errno::EBADMSG, Some(&format!("node is not a branch: 0x{:x}", self.type_)), None));
        }
        BranchCursor::new(self.pkt, self.base)
    }

    // ---- Optional Guard ----

    /// Optional chaining helper.
    /// Returns `this` for a present node.
    /// NullCursor's returns `None`.
    pub fn opt(self) -> Option<NodeCursor> {
        Some(self)
    }

    // ---- Shallow Getter ----

    /// Reads the current node at shallow depth.
    ///
    /// Shallow contract:
    /// - leaf cursor  -> materialized leaf value
    /// - null cursor  -> null
    /// - branch cursor is handled by BranchCursor's
    pub fn get(&self) -> Result<ShallowNodeValue, MarshalError> {
        let t = self.type_;
        Ok(match t {
            // -- Scalars --
            t if t == ScalarType::U8 as u8 => NodeValue::Number(self._u8()? as f64),
            t if t == ScalarType::I8 as u8 => NodeValue::Number(self._i8()? as f64),
            t if t == ScalarType::U16 as u8 => NodeValue::Number(self._u16()? as f64),
            t if t == ScalarType::I16 as u8 => NodeValue::Number(self._i16()? as f64),
            t if t == ScalarType::U32 as u8 => NodeValue::Number(self._u32()? as f64),
            t if t == ScalarType::I32 as u8 => NodeValue::Number(self._i32()? as f64),
            t if t == ScalarType::U64 as u8 => NodeValue::BigInt(self._u64()? as i128),
            t if t == ScalarType::I64 as u8 => NodeValue::BigInt(self._i64()? as i128),
            t if t == ScalarType::F32 as u8 => NodeValue::Number(self._f32()? as f64),
            t if t == ScalarType::F64 as u8 => NodeValue::Number(self._f64()?),
            t if t == ScalarType::BOOL as u8 => NodeValue::Bool(self._bool()?),

            // -- Sequences --
            t if t == SequenceType::U8_ARRAY as u8 => NodeValue::Array(self._u8_array()?.view()),
            t if t == SequenceType::I8_ARRAY as u8 => NodeValue::Array(self._i8_array()?.view()),
            t if t == SequenceType::U16_ARRAY as u8 => NodeValue::Array(self._u16_array()?.view()),
            t if t == SequenceType::I16_ARRAY as u8 => NodeValue::Array(self._i16_array()?.view()),
            t if t == SequenceType::U32_ARRAY as u8 => NodeValue::Array(self._u32_array()?.view()),
            t if t == SequenceType::I32_ARRAY as u8 => NodeValue::Array(self._i32_array()?.view()),
            t if t == SequenceType::U64_ARRAY as u8 => NodeValue::Array(self._u64_array()?.view()),
            t if t == SequenceType::I64_ARRAY as u8 => NodeValue::Array(self._i64_array()?.view()),
            t if t == SequenceType::F32_ARRAY as u8 => NodeValue::Array(self._f32_array()?.view()),
            t if t == SequenceType::F64_ARRAY as u8 => NodeValue::Array(self._f64_array()?.view()),
            t if t == SequenceType::BITSET as u8 => NodeValue::Array(self._bitset()?.view()),
            t if t == SequenceType::STR as u8 => NodeValue::Str(self._str()?),

            _ => return Err(MarshalError::new(Errno::EBADMSG, Some(&format!("unknown node type 0x{t:x}")), None)),
        })
    }

    // ---- Internal Leaf Readers ----

    fn scalar<const N: usize>(&self) -> Result<[u8; N], MarshalError> {
        if self.base as usize + N > self.pkt.byte_length() {
            return Err(MarshalError::new(Errno::EBADMSG, Some("scalar OOB"), None));
        }
        Ok(self.pkt.as_slice()[self.base as usize..self.base as usize + N].try_into().unwrap())
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
    fn _array<T: AnyElement>(&self) -> Result<TypedArray<T>, MarshalError> {
        if self.base + WORD_SZ > self.pkt.byte_length() as u32 {
            return Err(MarshalError::new(Errno::EBADMSG, Some("len slot OOB"), None));
        }
        let mut reg = WORD_REG;
        let [len, _] = GET_WORD(&self.pkt, self.base, &mut reg);
        let payload_start = self.base + WORD_SZ;

        let elem_sz = T::BYTES_PER_ELEMENT as u32;
        let payload_sz = len.wrapping_mul(elem_sz);

        if len != 0 && payload_sz / elem_sz != len {
            return Err(MarshalError::new(Errno::EBADMSG, Some("payload size overflow"), None));
        }

        if payload_start > self.pkt.byte_length() as u32 {
            return Err(MarshalError::new(Errno::EBADMSG, Some("payload OOB"), None));
        }
        if payload_sz > self.pkt.byte_length() as u32 - payload_start {
            return Err(MarshalError::new(Errno::EBADMSG, Some("payload OOB"), None));
        }

        Ok(TypedArray::over(self.pkt.buffer().clone(), self.pkt.byte_offset() + payload_start as usize, len as usize))
    }

    fn _u8_array(&self) -> Result<TypedArray<u8>, MarshalError> {
        self._array()
    }
    fn _i8_array(&self) -> Result<TypedArray<i8>, MarshalError> {
        self._array()
    }
    fn _u16_array(&self) -> Result<TypedArray<u16>, MarshalError> {
        self._array()
    }
    fn _i16_array(&self) -> Result<TypedArray<i16>, MarshalError> {
        self._array()
    }
    fn _u32_array(&self) -> Result<TypedArray<u32>, MarshalError> {
        self._array()
    }
    fn _i32_array(&self) -> Result<TypedArray<i32>, MarshalError> {
        self._array()
    }
    fn _u64_array(&self) -> Result<TypedArray<u64>, MarshalError> {
        self._array()
    }
    fn _i64_array(&self) -> Result<TypedArray<i64>, MarshalError> {
        self._array()
    }
    fn _f32_array(&self) -> Result<TypedArray<f32>, MarshalError> {
        self._array()
    }
    fn _f64_array(&self) -> Result<TypedArray<f64>, MarshalError> {
        self._array()
    }

    fn _bitset(&self) -> Result<U8Array, MarshalError> {
        if self.base + WORD_SZ > self.pkt.byte_length() as u32 {
            return Err(MarshalError::new(Errno::EBADMSG, Some("len slot OOB"), None));
        }
        let mut reg = WORD_REG;
        let [len, _] = GET_WORD(&self.pkt, self.base, &mut reg);
        let payload_start = self.base + WORD_SZ;

        let payload_sz = (len + 7) >> 3;
        if payload_start > self.pkt.byte_length() as u32 {
            return Err(MarshalError::new(Errno::EBADMSG, Some("payload OOB"), None));
        }
        if payload_sz > self.pkt.byte_length() as u32 - payload_start {
            return Err(MarshalError::new(Errno::EBADMSG, Some("payload OOB"), None));
        }

        let arr: U8Array = TypedArray::new(len as usize);
        for i in 0..len as usize {
            let x = self.pkt[payload_start as usize + (i >> 3)] & (1 << (i & 7));
            arr.put(i, if x != 0 { 1 } else { 0 });
        }
        Ok(arr)
    }

    // STR stores UTF-8 bytes with a trailing NUL on wire, while len excludes that terminator.
    fn _str(&self) -> Result<String, MarshalError> {
        if self.base + WORD_SZ > self.pkt.byte_length() as u32 {
            return Err(MarshalError::new(Errno::EBADMSG, Some("len slot OOB"), None));
        }
        let mut reg = WORD_REG;
        let [nbyte, _] = GET_WORD(&self.pkt, self.base, &mut reg);
        let payload_start = self.base + WORD_SZ;

        if payload_start > self.pkt.byte_length() as u32 {
            return Err(MarshalError::new(Errno::EBADMSG, Some("payload OOB"), None));
        }
        if nbyte + 1 > self.pkt.byte_length() as u32 - payload_start {
            return Err(MarshalError::new(Errno::EBADMSG, Some("payload OOB"), None));
        }
        if self.pkt[(payload_start + nbyte) as usize] != 0 {
            return Err(MarshalError::new(Errno::EBADMSG, Some("missing string terminator"), None));
        }

        Ok(encoding::td.decode(&self.pkt.subarray(payload_start as usize, (payload_start + nbyte) as usize)))
    }
}

#[derive(Clone, Debug)]
pub struct NullCursor(pub NodeCursor);

impl NullCursor {
    /// - `pkt`: source packet bytes
    pub fn new(pkt: U8Array, type_: NodeType) -> NullCursor {
        NullCursor(NodeCursor::new(pkt, NONE, type_))
    }

    // NullCursor preserves original node type metadata,
    // but is never considered a usable structural cursor.
    pub fn is_branch(&self) -> bool {
        false
    }

    /// Reinterpret current node as a branch cursor.
    /// Fails if the current node is not a branch.
    pub fn as_branch(self) -> Result<BranchCursor, MarshalError> {
        Err(MarshalError::new(Errno::EFAULT, Some("cannot cast a null node to a branch"), None))
    }

    // ---- Nullable Guard ----

    /// Optional chaining helper.
    /// Returns `None` for a physically absent node and `this` otherwise.
    pub fn opt(self) -> Option<NullCursor> {
        None
    }

    // ---- Shallow Getter ----

    /// Shallow read of a physically absent node reconstructs generic null.
    pub fn get(&self) -> ShallowNodeValue {
        // generic optional reconstruction
        NodeValue::Null
    }
}

#[derive(Clone, Debug)]
pub struct BranchCursor {
    pub cursor: NodeCursor,

    pub len: u32,
    pub child_start: u32, // first byte where child payloads may begin
}

impl PartialEq for BranchCursor {
    fn eq(&self, other: &BranchCursor) -> bool {
        self.cursor.pkt.is(&other.cursor.pkt) && self.cursor.base == other.cursor.base
    }
}

impl BranchCursor {
    pub fn new(pkt: U8Array, base: u32) -> Result<BranchCursor, MarshalError> {
        let cursor = NodeCursor::new(pkt, base, SpecialType::BRANCH as u8);
        let pkt = &cursor.pkt;

        if base + WORD_SZ > pkt.byte_length() as u32 {
            return Err(MarshalError::new(Errno::EBADMSG, Some("branch header OOB"), None));
        }
        let mut reg = WORD_REG;
        let [len, _] = GET_WORD(pkt, base, &mut reg);

        let table_off = base + WORD_SZ;
        if len > (pkt.byte_length() as u32 - table_off) / WORD_SZ {
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

    pub fn as_branch(self) -> Result<BranchCursor, MarshalError> {
        Ok(self)
    }

    pub fn opt(self) -> Option<BranchCursor> {
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
    pub fn get(&self) -> Result<ShallowNodeValue, MarshalError> {
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
    pub fn get_at(&self, idx: u32) -> Result<ShallowNodeValue, MarshalError> {
        self.at(idx)?.get()
    }

    /// Deep branch read.
    ///
    /// Recursively materializes the entire child subtree.
    pub fn get_deep(&self) -> Result<NodeValue, MarshalError> {
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
    pub fn at(&self, idx: u32) -> Result<Cursor, MarshalError> {
        if idx >= self.len {
            return Err(MarshalError::new(Errno::EFAULT, Some(&format!("child index out of bounds: {idx}")), None));
        }
        let entry_off = ENTRY_OFF(self.cursor.base, idx);

        let mut reg = WORD_REG;
        let [rel_off, desc] = GET_WORD(&self.cursor.pkt, entry_off, &mut reg);
        let type_: NodeType = field_get(desc, DESC_TYPE_SHAMT, DESC_TYPE_MASK) as NodeType;

        if rel_off == 0 {
            return Ok(Cursor::Null(NullCursor::new(self.cursor.pkt.clone(), type_)));
        }
        if rel_off > self.cursor.pkt.byte_length() as u32 - self.cursor.base {
            return Err(MarshalError::new(Errno::EBADMSG, Some("child offset overflow"), None));
        }
        let base = self.cursor.base + rel_off;
        if base < self.child_start || base >= self.cursor.pkt.byte_length() as u32 {
            return Err(MarshalError::new(Errno::EBADMSG, Some(&format!("bad child offset: base={} rel_off={rel_off}", self.cursor.base)), None));
        }
        CURSOR(&self.cursor.pkt, base, type_)
    }

    pub fn at_branch(&self, idx: u32) -> Result<BranchCursor, MarshalError> {
        self.at(idx)?.as_branch()
    }

    /// Returns all direct child cursors without materializing them.
    pub fn children(&self) -> Result<Vec<Cursor>, MarshalError> {
        (0..self.len).map(|i| self.at(i)).collect()
    }

    /// The children in order, as the cursor iterates there.
    pub fn iter(&self) -> impl Iterator<Item = Result<Cursor, MarshalError>> + '_ {
        (0..self.len).map(move |i| self.at(i))
    }
}

/// A typed-array value's view, for a caller that knows the element.
pub fn array_of<T: AnyElement>(v: &ArrayBufferView) -> Option<TypedArray<T>> {
    TypedArray::of(v)
}
