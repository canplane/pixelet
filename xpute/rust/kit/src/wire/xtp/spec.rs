// xpute-kit/wire/xtp/spec.rs

//! XTP's constants and node model.

// Stateless helpers are uppercase, as the C header's macros they mirror.
#![allow(non_snake_case, non_camel_case_types, clippy::identity_op)]

use crate::status::errno::Errno;
use crate::status::error::MarshalError;
#[derive(Clone, Debug, PartialEq)]
pub struct Array<'a> {
    pub type_: SequenceType,
    pub bytes: std::borrow::Cow<'a, [u8]>,
}

// A leaf type byte:
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

const ARG_SHAMT: u8 = 0;
const CLASS_SHAMT: u8 = ARG_SHAMT + ARG_W;
const SEQ_SHAMT: u8 = CLASS_SHAMT + CLASS_W;
const LEAF_SHAMT: u8 = 7;

pub const ARG_MASK: u8 = (1 << ARG_W) - 1;
const CLASS_MASK: u8 = ((1 << CLASS_W) - 1) << CLASS_SHAMT;

const BIT_SEQ: u8 = 1 << SEQ_SHAMT;
const BIT_LEAF: u8 = 1 << LEAF_SHAMT;

const CLASS_U: u8 = 0 << CLASS_SHAMT;
const CLASS_I: u8 = 1 << CLASS_SHAMT;
const CLASS_F: u8 = 2 << CLASS_SHAMT;
const CLASS_R: u8 = 3 << CLASS_SHAMT;

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

pub fn I(w: u32, sign: u8) -> u8 {
    BIT_LEAF | ((if sign != 0 { CLASS_I } else { CLASS_U }) | (w as u8 & ARG_MASK))
}
pub fn F(w: u32) -> u8 {
    BIT_LEAF | (CLASS_F | (w as u8 & ARG_MASK))
}

pub fn R(x: u32) -> u8 {
    BIT_LEAF | (CLASS_R | (x as u8 & ARG_MASK))
}

pub type NodeType = u8;

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
    /// `len` + 1 u32 offsets into the UTF-8 that follows, so a string costs
    /// no node.
    STRS = 0x80 | 32 | (24 | 2), // BIT_SEQ | R(2)
}

/// Types with the leaf bit clear.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum SpecialType {
    NIL = 0x00,
    BRANCH = 0x20,
    GRAFT = 0x40,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Never {}

/// `T` is a view on the write side, a branch cursor on the read side.
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

pub type LeafNode<'a> = Node<'a>;

#[derive(Clone, Debug, PartialEq)]
pub struct ScalarNode {
    pub type_: ScalarType,

    pub val: Option<Numeric>,
}

#[derive(Clone, Debug, PartialEq)]
pub enum SequenceVal<'a> {
    Str(String),
    Strs(Vec<String>),
    Array(Array<'a>),
}

#[derive(Clone, Debug, PartialEq)]
pub struct SequenceNode<'a> {
    pub type_: SequenceType,

    pub val: Option<SequenceVal<'a>>,
}

pub type SpecialNode<'a> = Node<'a>;

#[derive(Clone, Debug, PartialEq)]
pub struct BranchNode<'a> {
    pub val: Option<Vec<Node<'a>>>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct GraftNode<'a> {
    pub val: &'a [u8],
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct NilNode;

pub const NIL_NODE: Node = Node::Nil(NilNode);

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

/// Reserved zero on the wire; also the encoder's uninitialized sentinel.
pub const NONE: u32 = 0;

pub const RESERVED: u32 = 0;

pub const LE: bool = true;

pub const MAGIC: u32 = 0x00505458; // "XTP\0" little-endian word

#[derive(Clone, Copy, Debug)]
pub struct EncodingState {
    pub base: u32,
    pub lim: u32,
    pub type_: NodeType,
}

pub const DESC_TYPE_W: u32 = 8;
pub const DESC_TYPE_SHAMT: u32 = 0;
pub const DESC_TYPE_MASK: u32 = (1 << DESC_TYPE_W) - 1;

pub const WORD_SZ: AlignUnit = 8;

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

pub type AlignUnit = u32;

/// `unit` is a power of two no larger than WORD_SZ.
pub fn ALIGN(nbyte: u32, unit: AlignUnit) -> Result<u32, MarshalError> {
    if unit == 0 || (unit & unit.wrapping_sub(1)) != 0 || unit > WORD_SZ {
        return Err(MarshalError::new(Errno::EINVAL));
    }

    let mask = unit - 1;
    if nbyte > u32::MAX - mask {
        return Err(MarshalError::new(Errno::EOVERFLOW));
    }
    Ok((nbyte + mask) & !mask)
}

/// A BITSET's value is one byte a bit before it is packed.
pub fn ELEM_SZ(type_: SequenceType) -> AlignUnit {
    match type_ {
        SequenceType::U8_ARRAY | SequenceType::I8_ARRAY | SequenceType::BITSET | SequenceType::STR => 1,
        SequenceType::U16_ARRAY | SequenceType::I16_ARRAY => 2,
        SequenceType::U32_ARRAY | SequenceType::I32_ARRAY | SequenceType::F32_ARRAY | SequenceType::STRS => 4,
        SequenceType::U64_ARRAY | SequenceType::I64_ARRAY | SequenceType::F64_ARRAY => 8,
    }
}

pub fn ALIGN_SZ(type_: u8) -> Result<AlignUnit, MarshalError> {
    // A sequence or branch aligns to WORD_SZ so its len word does.
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

        _ => return Err(MarshalError::new(Errno::EBADMSG)),
    })
}

/// Must stay a multiple of WORD_SZ.
pub const HDR_SZ: u32 = WORD_SZ * 2;

pub const MAX_PKT_SZ: u32 = 0x7fffffff; // 2GB - 1
