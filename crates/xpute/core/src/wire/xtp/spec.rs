// xpute-core/wire/xtp/spec.rs

//! Relocatable tree packet format.
//!
//! Scope
//! -----
//! - Defines a relocatable packet format for tree-shaped payloads.
//! - Each packet contains a fixed-size header and one encoded root subtree payload.
//! - Internal subtree references are position-independent through relative offsets.
//!
//! Model
//! -----
//! - Scalar leaves are headerless payloads.
//! - Sequence leaves are encoded as [len slot | payload], with payload starting
//!   immediately after the len slot.
//! - Sequence payload alignment is derived from the WORD-aligned node base.
//! - Branch nodes are encoded as [len slot | offset table].
//! - Graft nodes splice a previously encoded subtree packet.
//! - Physical null at the packet root is encoded by preserving the node type and
//!   writing root_payload_sz = 0.
//! - Physical null at a branch child is encoded by preserving the child type and
//!   writing child_rel_off = 0.
//! - Generic null lowering uses the NIL special node.
//!
//! Semantics
//! ---------
//! - This is a relocatable tree packet format, not a self-delimiting stream format.
//! - The format itself does not define schema semantics.
//! - Higher layers define field meaning, ordering rules, and validation policy.
//! - Graft is a splice primitive, not an independent payload class.
//! - A grafted subtree must produce the same root payload layout as if encoded inline.
//! - Typed optional values preserve their original node kind even when physically absent.
//! - Therefore `str(null)`, `u32(null)`, `branch(null)`, and generic `set(null)` / `put(null)` are distinct lowerings.
//!
//! Design Goals
//! ------------
//! - Relocatable tree packet layout
//! - Lazy cursor-style traversal
//! - WORD-aligned structural-node discipline
//! - sequence payload alignment derived from fixed [len slot | payload] layout
//! - len/off/size read from the low 32 bits of an 8-byte slot, the high
//!   32 reserved, so a slot is one word and a field is one register
//!
//! Optional model:
//! - packet root physical absence = root_payload_sz == 0 with preserved node type
//! - branch child physical absence = child_rel_off == 0 with preserved child type
//! - generic set(null) / put(null) lower to NIL
//! - typed writers preserve their original node kind
//! - NullCursor preserves type metadata, while get() reconstructs null and opt() returns undefined
//!
//! Format contracts:
//! - wire byte order is little-endian for all multi-byte scalar values and len/off/size word fields
//! - all len/off fields occupy one 8-byte slot
//! - len/off/size semantics currently use only the low 32 bits
//! - high 32 bits are reserved for future use
//! - branch and sequence nodes are aligned to at least WORD_SZ
//! - final packet size is aligned to WORD_SZ
//! - every node type is exactly 1 byte wide in the format
//! - the header type field occupies bits 0..7 of a 32-bit slot
//! - bits 8..31 of that slot are reserved
//!
//! Packet header:
//! - packet header is two 8-byte words:
//!   - word0: [magic 32 | reserved 32]
//!   - word1: [root_payload_sz 32 | type 8 | reserved 24]
//! - HDR_SZ is required to be aligned to WORD_SZ
//! - therefore a present root payload always starts exactly at HDR_SZ
//! - root_payload_sz is the byte size of the encoded root payload region,
//!   excluding the fixed packet header
//! - root_payload_sz is constrained to:
//!   - 0                    : physical null root
//!   - 1..(packet_size-HDR_SZ) : present root payload
//! - type is always preserved even when root_payload_sz == 0
//! - therefore a physically null root may still carry typed metadata
//!
//! Branch child entry:
//! - each child entry is one 8-byte word pair:
//!   - lo32: child_rel_off
//!   - hi32: type in bits 0..7, remaining bits reserved
//! - child_rel_off is relative to the enclosing branch base
//! - child_rel_off == 0 means physical null while preserving child type
//! - child_rel_off != 0 must satisfy:
//!   - child_base = branch_base + child_rel_off
//!   - child_base >= table_end
//!   - child_base < packet_size
//! - child_rel_off must not point into:
//!   - the branch header (base .. base + WORD_SZ)
//!   - the offset table (base + WORD_SZ .. table_end)
//! - any violation is a malformed packet
//!
//! `NodeType` is the byte every node type is on the wire, `Node` the node
//! kinds, and `NodeValue<T>` the value domain with `T` as its extra variant:
//! a view on the write side, a branch cursor on the read side.

// Uppercase throughout, as the notation these operations are written in: one
// that holds no state and closes over nothing is named the way a C header
// names its macros. Rust's own answer to a macro like that is a `const fn`,
// which it cases snake, so the notation and the language disagree here and
// the file keeps the notation — suspended once for the file, which is the
// level the choice is made at rather than item by item.
#![allow(non_snake_case, non_camel_case_types, clippy::identity_op)]

use crate::status::errno::Errno;
use crate::status::error::MarshalError;
/// A sequence leaf's elements: the type it is written as, and its bytes.
#[derive(Clone, Debug, PartialEq)]
pub struct Array<'a> {
    pub type_: SequenceType,
    pub bytes: std::borrow::Cow<'a, [u8]>,
}

// ============ Type Encoding ============
//
// leaf type layout (when L = 1)
//
// 7 | 6 | 5 | 4 | 3 | 2 1 0
// L | R | S | C | C |   A
//
// L: leaf bit
// R: reserved
// S: sequence bit                (0 = scalar, 1 = sequence)
// C: class bits                  (00 = u, 01 = i, 10 = f, 11 = non-numeric)
// A: 3-bit argument field

const ARG_W: u8 = 3;
const CLASS_W: u8 = 2;

// positions
const ARG_SHAMT: u8 = 0;
const CLASS_SHAMT: u8 = ARG_SHAMT + ARG_W; // bits 3..4
const SEQ_SHAMT: u8 = CLASS_SHAMT + CLASS_W; // bit 5
const LEAF_SHAMT: u8 = 7; // bit 7

// widths & masks
pub const ARG_MASK: u8 = (1 << ARG_W) - 1;
const CLASS_MASK: u8 = ((1 << CLASS_W) - 1) << CLASS_SHAMT;

// node bits
const BIT_SEQ: u8 = 1 << SEQ_SHAMT;
const BIT_LEAF: u8 = 1 << LEAF_SHAMT;

// leaf class tags
const CLASS_U: u8 = 0 << CLASS_SHAMT;
const CLASS_I: u8 = 1 << CLASS_SHAMT;
const CLASS_F: u8 = 2 << CLASS_SHAMT;
const CLASS_R: u8 = 3 << CLASS_SHAMT;

// ---- Extract Helpers ----

pub fn TYPE_ARG_OF(type_: u8) -> u8 {
    type_ & ARG_MASK
}

pub fn TYPE_IS_LEAF(type_: u8) -> bool {
    (type_ & BIT_LEAF) != 0
}
pub fn TYPE_IS_SPECIAL(type_: u8) -> bool {
    (type_ & BIT_LEAF) == 0
}

pub fn TYPE_IS_SCALAR(type_: u8) -> bool {
    TYPE_IS_LEAF(type_) && (type_ & BIT_SEQ) == 0
}
pub fn TYPE_IS_SEQ(type_: u8) -> bool {
    TYPE_IS_LEAF(type_) && (type_ & BIT_SEQ) != 0
}

pub fn TYPE_IS_NONNUM(type_: u8) -> bool {
    TYPE_IS_LEAF(type_) && (type_ & CLASS_MASK) == CLASS_R
}
pub fn TYPE_IS_FLOAT(type_: u8) -> bool {
    TYPE_IS_LEAF(type_) && (type_ & CLASS_MASK) == CLASS_F
}
pub fn TYPE_IS_SIGNED(type_: u8) -> bool {
    TYPE_IS_LEAF(type_) && (type_ & CLASS_MASK) == CLASS_I
}

// ---- Type Constructors ----

// numeric pack:
pub fn I(w: u32, sign: u8) -> u8 {
    BIT_LEAF | ((if sign != 0 { CLASS_I } else { CLASS_U }) | (w as u8 & ARG_MASK))
}
pub fn F(w: u32) -> u8 {
    BIT_LEAF | (CLASS_F | (w as u8 & ARG_MASK))
}

// non-numeric pack:
pub fn R(x: u32) -> u8 {
    BIT_LEAF | (CLASS_R | (x as u8 & ARG_MASK))
}

// ---- Type Tables ----

/// A node type as the wire has it: a leaf type or a special type.
pub type NodeType = u8;

/// A leaf type: a scalar type or a sequence type.
pub type LeafType = u8;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum ScalarType {
    U8 = 0x80 | 0,         // I(0, 0)
    I8 = 0x80 | 8,         // I(0, 1)
    U16 = 0x80 | 1,        // I(1, 0)
    I16 = 0x80 | (8 | 1),  // I(1, 1)
    U32 = 0x80 | 2,        // I(2, 0)
    I32 = 0x80 | (8 | 2),  // I(2, 1)
    U64 = 0x80 | 3,        // I(3, 0)
    I64 = 0x80 | (8 | 3),  // I(3, 1)
    F32 = 0x80 | (16 | 2), // F(2)
    F64 = 0x80 | (16 | 3), // F(3)

    BOOL = 0x80 | 24, // R(0)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum SequenceType {
    U8_ARRAY = 0x80 | 32,             // BIT_SEQ | I(0, 0)
    I8_ARRAY = 0x80 | 32 | 8,         // BIT_SEQ | I(0, 1)
    U16_ARRAY = 0x80 | 32 | 1,        // BIT_SEQ | I(1, 0)
    I16_ARRAY = 0x80 | 32 | (8 | 1),  // BIT_SEQ | I(1, 1)
    U32_ARRAY = 0x80 | 32 | 2,        // BIT_SEQ | I(2, 0)
    I32_ARRAY = 0x80 | 32 | (8 | 2),  // BIT_SEQ | I(2, 1)
    U64_ARRAY = 0x80 | 32 | 3,        // BIT_SEQ | I(3, 0)
    I64_ARRAY = 0x80 | 32 | (8 | 3),  // BIT_SEQ | I(3, 1)
    F32_ARRAY = 0x80 | 32 | (16 | 2), // BIT_SEQ | F(2)
    F64_ARRAY = 0x80 | 32 | (16 | 3), // BIT_SEQ | F(3)

    BITSET = 0x80 | 32 | 24,    // BIT_SEQ | R(0)
    STR = 0x80 | 32 | (24 | 1), // BIT_SEQ | R(1)
}

// special type space (when L = 0)
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum SpecialType {
    NIL = 0x00, // generic/untyped null node

    // structural node
    BRANCH = 0x20, // 0010 0000

    // subtree splice primitive
    GRAFT = 0x40, // 0100 0000
}

// ============ Model ============

/// The type-level `never`: a `NodeValue` with no extra variant.
#[derive(Clone, Debug, PartialEq)]
pub enum Never {}

// generic input/output value domain used by tree views and cursors
// plain object lowering is intentionally excluded for now
#[derive(Clone, Debug, PartialEq)]
pub enum NodeValue<'a, T = Never> {
    Extra(T),
    Null,
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
    Array(Array<'a>),
    List(Vec<NodeValue<'a, T>>),
}

/// A scalar leaf's value, at the width its `ScalarType` names.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Numeric {
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
}

#[derive(Clone, Debug, PartialEq)]
pub enum Node<'a> {
    Scalar(ScalarNode),
    Sequence(SequenceNode<'a>),
    Nil(NilNode),
    Branch(BranchNode<'a>),
    Graft(GraftNode<'a>),
}

impl Node<'_> {
    /// The node's type byte.
    pub fn type_(&self) -> NodeType {
        match self {
            Node::Scalar(n) => n.type_ as u8,
            Node::Sequence(n) => n.type_ as u8,
            Node::Nil(_) => SpecialType::NIL as u8,
            Node::Branch(_) => SpecialType::BRANCH as u8,
            Node::Graft(_) => SpecialType::GRAFT as u8,
        }
    }
}

// ---- Leaf Nodes ----

/// A leaf node: a scalar node or a sequence node.
pub type LeafNode<'a> = Node<'a>;

#[derive(Clone, Debug, PartialEq)]
pub struct ScalarNode {
    pub type_: ScalarType,

    pub val: Option<Numeric>,
}

/// A sequence node's value.
#[derive(Clone, Debug, PartialEq)]
pub enum SequenceVal<'a> {
    Str(String),
    Array(Array<'a>),
}

#[derive(Clone, Debug, PartialEq)]
pub struct SequenceNode<'a> {
    pub type_: SequenceType,

    pub val: Option<SequenceVal<'a>>,
}

// ---- Special Nodes ----

/// A special node: nil, a branch or a graft.
pub type SpecialNode<'a> = Node<'a>;

#[derive(Clone, Debug, PartialEq)]
pub struct BranchNode<'a> {
    pub val: Option<Vec<Node<'a>>>,
}

// special splice primitive for a previously encoded subtree packet
#[derive(Clone, Debug, PartialEq)]
pub struct GraftNode<'a> {
    pub val: &'a [u8],
}

// ---- Optional ----

// Optional encoding model:
// - optional means the node kind is preserved while payload is absent
// - packet-root physical absence is encoded as root_payload_sz == 0
// - branch-child physical absence is encoded as child_rel_off == 0
// - generic set(null) / put(null) lower to NIL
// - typed writers preserve their original node kind
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct NilNode;

pub const NIL_NODE: Node = Node::Nil(NilNode);

// ---- Predicates ----

pub fn NODE_IS_LEAF(node: &Node) -> bool {
    TYPE_IS_LEAF(node.type_())
}
pub fn NODE_IS_SCALAR(node: &Node) -> bool {
    TYPE_IS_SCALAR(node.type_())
}
pub fn NODE_IS_SEQ(node: &Node) -> bool {
    TYPE_IS_SEQ(node.type_())
}

pub fn NODE_IS_SPECIAL(node: &Node) -> bool {
    TYPE_IS_SPECIAL(node.type_())
}
pub fn NODE_IS_NIL(node: &Node) -> bool {
    node.type_() == SpecialType::NIL as u8
}
pub fn NODE_IS_BRANCH(node: &Node) -> bool {
    node.type_() == SpecialType::BRANCH as u8
}
pub fn NODE_IS_GRAFT(node: &Node) -> bool {
    node.type_() == SpecialType::GRAFT as u8
}

// ============ Wire Primitives ============

// ---- Sentinels ----

pub const NONE: u32 = 0; // reserved zero on wire; reused as local uninitialized sentinel

pub const RESERVED: u32 = 0;

// ---- Endianness / Magic ----

pub const LE: bool = true;

pub const MAGIC: u32 = 0x00505458; // "XTP\0" little-endian word

// ---- Descriptor Layout ----

// encoder work state for one node; updated progressively during encode
#[derive(Clone, Copy, Debug)]
pub struct EncodingState {
    pub base: u32,
    pub lim: u32,
    pub type_: NodeType,
}

// NONE is reused as the local uninitialized sentinel during encode.
pub const DESC_TYPE_W: u32 = 8;
pub const DESC_TYPE_SHAMT: u32 = 0;
pub const DESC_TYPE_MASK: u32 = (1 << DESC_TYPE_W) - 1;

// ---- Word Helpers ----

pub const WORD_SZ: AlignUnit = 8; // 8 bytes

pub fn SET_WORD(buf: &mut [u8], off: u32, lo32: u32, hi32: u32) {
    let at = off as usize;
    buf[at..at + 4].copy_from_slice(&lo32.to_le_bytes());
    buf[at + 4..at + 8].copy_from_slice(&hi32.to_le_bytes());
}
pub fn GET_WORD(buf: &[u8], off: u32, out: &mut [u32; 2]) -> [u32; 2] {
    let at = off as usize;
    out[0] = u32::from_le_bytes(buf[at..at + 4].try_into().unwrap());
    out[1] = u32::from_le_bytes(buf[at + 4..at + 8].try_into().unwrap());
    *out
}

// ============ Layout ============

// ---- Alignment ----

pub type AlignUnit = u32;

/// Rounds `nbyte` up to the next multiple of `unit`.
///
/// Requirements:
/// - `unit` must be non-zero
/// - `unit` must be a power of two
/// - `unit` must not exceed WORD_SZ
pub fn ALIGN(nbyte: u32, unit: AlignUnit) -> Result<u32, MarshalError> {
    if (unit & unit.wrapping_sub(1)) != 0 || unit > WORD_SZ {
        return Err(MarshalError::new(Errno::EINVAL, Some(&format!("ALIGN: bad unit {unit}")), None));
    }

    let mask = unit - 1;
    if nbyte > u32::MAX - mask {
        return Err(MarshalError::new(Errno::EOVERFLOW, Some(&format!("ALIGN: {nbyte} does not align within u32")), None));
    }
    Ok((nbyte + mask) & !mask)
}

// ---- Node Layout ----

/// A sequence leaf's element size in bytes. A BITSET's value is one byte a
/// bit before it is packed.
pub fn ELEM_SZ(type_: SequenceType) -> AlignUnit {
    match type_ {
        SequenceType::U8_ARRAY | SequenceType::I8_ARRAY | SequenceType::BITSET | SequenceType::STR => 1,
        SequenceType::U16_ARRAY | SequenceType::I16_ARRAY => 2,
        SequenceType::U32_ARRAY | SequenceType::I32_ARRAY | SequenceType::F32_ARRAY => 4,
        SequenceType::U64_ARRAY | SequenceType::I64_ARRAY | SequenceType::F64_ARRAY => 8,
    }
}

pub fn ALIGN_SZ(type_: u8) -> Result<AlignUnit, MarshalError> {
    // alignment rules (scalar leaf, sequence leaf, or branch only):
    //
    // scalar:
    //   align to element size
    //
    // sequence / branch:
    //   align to WORD_SZ so the leading len slot is naturally aligned
    //   and child tables remain word-aligned
    if type_ == SpecialType::BRANCH as u8 || TYPE_IS_SEQ(type_) {
        return Ok(WORD_SZ);
    }

    Ok(match type_ {
        t if t == ScalarType::U8 as u8 || t == ScalarType::I8 as u8 => 1,
        t if t == ScalarType::U16 as u8 || t == ScalarType::I16 as u8 => 2,
        t if t == ScalarType::U32 as u8 || t == ScalarType::I32 as u8 => 4,
        t if t == ScalarType::U64 as u8 || t == ScalarType::I64 as u8 => 8,
        t if t == ScalarType::F32 as u8 => 4,
        t if t == ScalarType::F64 as u8 => 8,

        t if t == ScalarType::BOOL as u8 => 1,

        _ => return Err(MarshalError::new(Errno::EBADMSG, Some(&format!("unknown type 0x{type_:x}")), None)),
    })
}

// ---- Packet Layout ----

pub const HDR_SZ: u32 = WORD_SZ * 2; // 16 bytes, must remain aligned to WORD_SZ

pub const MAX_PKT_SZ: u32 = 0x7fffffff; // 2GB - 1
