// @xpute/spec/io/marshal/tree/spec.rs

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
//! - JS-friendly low-32-bit len/off/size semantics in 8-byte slots

use crate::sys::errno;
use crate::sys::error::MarshalError;

pub type AlignUnit = u32;

pub const ARG_W: u8 = 3;
pub const CLASS_W: u8 = 2;

pub const ARG_SHAMT: u8 = 0;
pub const CLASS_SHAMT: u8 = ARG_SHAMT + ARG_W;
pub const SEQ_SHAMT: u8 = CLASS_SHAMT + CLASS_W;
pub const LEAF_SHAMT: u8 = 7;

pub const ARG_MASK: u8 = (1 << ARG_W) - 1;
pub const CLASS_MASK: u8 = ((1 << CLASS_W) - 1) << CLASS_SHAMT;

pub const BIT_SEQ: u8 = 1 << SEQ_SHAMT;
pub const BIT_LEAF: u8 = 1 << LEAF_SHAMT;

pub const CLASS_U: u8 = 0 << CLASS_SHAMT;
pub const CLASS_I: u8 = 1 << CLASS_SHAMT;
pub const CLASS_F: u8 = 2 << CLASS_SHAMT;
pub const CLASS_R: u8 = 3 << CLASS_SHAMT;

pub const NONE: u32 = 0;
pub const RESERVED: u32 = 0;
pub const LE: bool = true;
pub const MAGIC: u32 = 0x0050_5458;

pub const DESC_TYPE_W: u32 = 8;
pub const DESC_TYPE_SHAMT: u32 = 0;
pub const DESC_TYPE_MASK: u32 = (1 << DESC_TYPE_W) - 1;

pub const WORD_SZ: u32 = 8;
pub const HDR_SZ: u32 = WORD_SZ * 2;
pub const MAX_PKT_SZ: u32 = 0x7fff_ffff;

pub const fn i(w: u32, sign: bool) -> u8 {
    BIT_LEAF | ((if sign { CLASS_I } else { CLASS_U }) | ((w as u8) & ARG_MASK))
}
pub const fn f(w: u32) -> u8 {
    BIT_LEAF | (CLASS_F | ((w as u8) & ARG_MASK))
}
pub const fn r(x: u32) -> u8 {
    BIT_LEAF | (CLASS_R | ((x as u8) & ARG_MASK))
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum ScalarType {
    U8 = i(0, false),
    I8 = i(0, true),
    U16 = i(1, false),
    I16 = i(1, true),
    U32 = i(2, false),
    I32 = i(2, true),
    U64 = i(3, false),
    I64 = i(3, true),
    F32 = f(2),
    F64 = f(3),
    Bool = r(0),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum SequenceType {
    U8Array = BIT_LEAF | BIT_SEQ | CLASS_U,
    I8Array = BIT_LEAF | BIT_SEQ | CLASS_I,
    U16Array = BIT_LEAF | BIT_SEQ | (CLASS_U | 1),
    I16Array = BIT_LEAF | BIT_SEQ | (CLASS_I | 1),
    U32Array = BIT_LEAF | BIT_SEQ | (CLASS_U | 2),
    I32Array = BIT_LEAF | BIT_SEQ | (CLASS_I | 2),
    U64Array = BIT_LEAF | BIT_SEQ | (CLASS_U | 3),
    I64Array = BIT_LEAF | BIT_SEQ | (CLASS_I | 3),
    F32Array = BIT_LEAF | BIT_SEQ | (CLASS_F | 2),
    F64Array = BIT_LEAF | BIT_SEQ | (CLASS_F | 3),
    Bitset = BIT_LEAF | BIT_SEQ | CLASS_R,
    Str = BIT_LEAF | BIT_SEQ | (CLASS_R | 1),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum SpecialType {
    Nil = 0x00,
    Branch = 0x20,
    Graft = 0x40,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NodeType {
    Scalar(ScalarType),
    Sequence(SequenceType),
    Special(SpecialType),
}

#[derive(Clone, Debug, PartialEq)]
pub enum SequenceValue {
    U8Array(Vec<u8>),
    I8Array(Vec<i8>),
    U16Array(Vec<u16>),
    I16Array(Vec<i16>),
    U32Array(Vec<u32>),
    I32Array(Vec<i32>),
    U64Array(Vec<u64>),
    I64Array(Vec<i64>),
    F32Array(Vec<f32>),
    F64Array(Vec<f64>),
    Bitset(Vec<u8>),
    Str(String),
}

#[derive(Clone, Debug, PartialEq)]
pub enum Node {
    Nil,
    Scalar(ScalarType, Option<ScalarValue>),
    Sequence(SequenceType, Option<SequenceValue>),
    Branch(Option<Vec<Node>>),
    Graft(Vec<u8>),
}

#[derive(Clone, Debug, PartialEq)]
pub enum ScalarValue {
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
    Bool(bool),
}

#[derive(Clone, Debug, PartialEq)]
pub enum NodeValue {
    Null,
    Number(f64),
    BigInt(i128),
    Bool(bool),
    Str(String),
    U8Array(Vec<u8>),
    I8Array(Vec<i8>),
    U16Array(Vec<u16>),
    I16Array(Vec<i16>),
    U32Array(Vec<u32>),
    I32Array(Vec<i32>),
    U64Array(Vec<u64>),
    I64Array(Vec<i64>),
    F32Array(Vec<f32>),
    F64Array(Vec<f64>),
    Array(Vec<NodeValue>),
}

#[derive(Clone, Debug)]
pub struct EncodingState {
    pub base: u32,
    pub lim: u32,
    pub node_type: NodeType,
}

#[derive(Clone, Debug, PartialEq)]
pub enum CursorValue {
    Null,
    Scalar(ScalarValue),
    Sequence(SequenceValue),
    Branch(Vec<CursorValue>),
}

pub const NIL_NODE: Node = Node::Nil;

pub fn type_arg_of(node_type: u8) -> u8 { node_type & ARG_MASK }
pub fn type_is_leaf(node_type: u8) -> bool { (node_type & BIT_LEAF) != 0 }
pub fn type_is_special(node_type: u8) -> bool { !type_is_leaf(node_type) }
pub fn type_is_scalar(node_type: u8) -> bool { type_is_leaf(node_type) && (node_type & BIT_SEQ) == 0 }
pub fn type_is_seq(node_type: u8) -> bool { type_is_leaf(node_type) && (node_type & BIT_SEQ) != 0 }

pub fn node_is_leaf(node: &Node) -> bool { matches!(node, Node::Scalar(..) | Node::Sequence(..)) }
pub fn node_is_seq(node: &Node) -> bool { matches!(node, Node::Sequence(..)) }
pub fn node_is_branch(node: &Node) -> bool { matches!(node, Node::Branch(..)) }
pub fn node_is_graft(node: &Node) -> bool { matches!(node, Node::Graft(..)) }
pub fn node_is_nil(node: &Node) -> bool { matches!(node, Node::Nil) }

pub fn set_word(buf: &mut [u8], off: u32, lo32: u32, hi32: u32) {
    let off = off as usize;
    buf[off..off + 4].copy_from_slice(&lo32.to_le_bytes());
    buf[off + 4..off + 8].copy_from_slice(&hi32.to_le_bytes());
}

pub fn get_word(buf: &[u8], off: u32) -> Result<(u32, u32), MarshalError> {
    let off = off as usize;
    if off + WORD_SZ as usize > buf.len() {
        return Err(MarshalError::new(errno::EBADMSG, "word OOB"));
    }
    let lo = u32::from_le_bytes(buf[off..off + 4].try_into().expect("u32 slice"));
    let hi = u32::from_le_bytes(buf[off + 4..off + 8].try_into().expect("u32 slice"));
    Ok((lo, hi))
}

pub fn align(nbyte: u32, unit: AlignUnit) -> Result<u32, MarshalError> {
    if unit == 0 || (unit & (unit - 1)) != 0 || unit > WORD_SZ {
        return Err(MarshalError::new(errno::EINVAL, format!("ALIGN: bad unit {unit}")));
    }
    let mask = unit - 1;
    Ok((nbyte + mask) & !mask)
}

pub fn align_sz(node_type: NodeType) -> Result<AlignUnit, MarshalError> {
    match node_type {
        NodeType::Special(SpecialType::Branch) | NodeType::Sequence(_) => Ok(WORD_SZ),
        NodeType::Scalar(ScalarType::U8 | ScalarType::I8 | ScalarType::Bool) => Ok(1),
        NodeType::Scalar(ScalarType::U16 | ScalarType::I16) => Ok(2),
        NodeType::Scalar(ScalarType::U32 | ScalarType::I32 | ScalarType::F32) => Ok(4),
        NodeType::Scalar(ScalarType::U64 | ScalarType::I64 | ScalarType::F64) => Ok(8),
        _ => Err(MarshalError::new(errno::EBADMSG, format!("unknown type 0x{:x}", node_type_to_u8(node_type)))),
    }
}

pub fn node_type_to_u8(node_type: NodeType) -> u8 {
    match node_type {
        NodeType::Scalar(v) => v as u8,
        NodeType::Sequence(v) => v as u8,
        NodeType::Special(v) => v as u8,
    }
}

pub fn node_type_from_u8(value: u8) -> Result<NodeType, MarshalError> {
    let out = match value {
        x if x == ScalarType::U8 as u8 => NodeType::Scalar(ScalarType::U8),
        x if x == ScalarType::I8 as u8 => NodeType::Scalar(ScalarType::I8),
        x if x == ScalarType::U16 as u8 => NodeType::Scalar(ScalarType::U16),
        x if x == ScalarType::I16 as u8 => NodeType::Scalar(ScalarType::I16),
        x if x == ScalarType::U32 as u8 => NodeType::Scalar(ScalarType::U32),
        x if x == ScalarType::I32 as u8 => NodeType::Scalar(ScalarType::I32),
        x if x == ScalarType::U64 as u8 => NodeType::Scalar(ScalarType::U64),
        x if x == ScalarType::I64 as u8 => NodeType::Scalar(ScalarType::I64),
        x if x == ScalarType::F32 as u8 => NodeType::Scalar(ScalarType::F32),
        x if x == ScalarType::F64 as u8 => NodeType::Scalar(ScalarType::F64),
        x if x == ScalarType::Bool as u8 => NodeType::Scalar(ScalarType::Bool),
        x if x == SequenceType::U8Array as u8 => NodeType::Sequence(SequenceType::U8Array),
        x if x == SequenceType::I8Array as u8 => NodeType::Sequence(SequenceType::I8Array),
        x if x == SequenceType::U16Array as u8 => NodeType::Sequence(SequenceType::U16Array),
        x if x == SequenceType::I16Array as u8 => NodeType::Sequence(SequenceType::I16Array),
        x if x == SequenceType::U32Array as u8 => NodeType::Sequence(SequenceType::U32Array),
        x if x == SequenceType::I32Array as u8 => NodeType::Sequence(SequenceType::I32Array),
        x if x == SequenceType::U64Array as u8 => NodeType::Sequence(SequenceType::U64Array),
        x if x == SequenceType::I64Array as u8 => NodeType::Sequence(SequenceType::I64Array),
        x if x == SequenceType::F32Array as u8 => NodeType::Sequence(SequenceType::F32Array),
        x if x == SequenceType::F64Array as u8 => NodeType::Sequence(SequenceType::F64Array),
        x if x == SequenceType::Bitset as u8 => NodeType::Sequence(SequenceType::Bitset),
        x if x == SequenceType::Str as u8 => NodeType::Sequence(SequenceType::Str),
        x if x == SpecialType::Nil as u8 => NodeType::Special(SpecialType::Nil),
        x if x == SpecialType::Branch as u8 => NodeType::Special(SpecialType::Branch),
        x if x == SpecialType::Graft as u8 => NodeType::Special(SpecialType::Graft),
        _ => return Err(MarshalError::new(errno::EBADMSG, format!("unknown type 0x{value:x}"))),
    };
    Ok(out)
}
