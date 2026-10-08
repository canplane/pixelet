// @xpute/kit/wire/xtp/spec.ts

// Relocatable tree packet: a 16-byte header and one root subtree, with children
// addressed by offsets relative to their branch, so a packet can be grafted
// anywhere unchanged. Little-endian; every len/off/size field is an 8-byte slot
// of which only the low 32 bits are used.
//
// Header: word0 = [magic 32 | reserved 32], word1 = [root_payload_sz 32 | type 8 | reserved 24].
// Branch: [len slot | child table], each entry [child_rel_off 32 | type 8 | reserved 24].
// A typed null keeps its type: root_payload_sz == 0 at the root, child_rel_off == 0
// in a branch. A nonzero child_rel_off must land at or past the table's end and
// inside the packet, or the packet is malformed.

import type { numeric, primitive, u32, u8 } from "@xpute/kit/abi/word.ts";
import type { BigTypedArray, TypedArray, U8Array } from "@xpute/kit/abi/array.ts";
import type { Nullable } from "@xpute/kit/lang/type.ts";
import { Errno } from "@xpute/kit/status/errno.spec.ts";
import { MarshalError } from "@xpute/kit/status/error.ts";

// Leaf type byte: bit 7 leaf, bit 5 sequence, bits 3..4 class (u, i, f,
// non-numeric), bits 0..2 argument (log2 width for numbers).

const ARG_W = 3;
const CLASS_W = 2;

const ARG_SHAMT: u8 = 0;
const CLASS_SHAMT: u8 = ARG_SHAMT + ARG_W; // bits 3..4
const SEQ_SHAMT: u8 = CLASS_SHAMT + CLASS_W; // bit 5
const LEAF_SHAMT: u8 = 7; // bit 7

export const ARG_MASK: u8 = ((1 << ARG_W) - 1) as u8;
const CLASS_MASK: u8 = (((1 << CLASS_W) - 1) << CLASS_SHAMT) as u8;

const BIT_SEQ: u8 = (1 << SEQ_SHAMT) as u8;
const BIT_LEAF: u8 = (1 << LEAF_SHAMT) as u8;

const CLASS_U: u8 = (0 << CLASS_SHAMT) as u8;
const CLASS_I: u8 = (1 << CLASS_SHAMT) as u8;
const CLASS_F: u8 = (2 << CLASS_SHAMT) as u8;
const CLASS_R: u8 = (3 << CLASS_SHAMT) as u8;

export const TYPE_ARG_OF = (type: u8): u8 => (type & ARG_MASK) as u8;

export const TYPE_IS_LEAF = (type: u8): type is LeafType => (type & BIT_LEAF) !== 0;
export const TYPE_IS_SPECIAL = (type: u8): type is SpecialType => (type & BIT_LEAF) === 0;

export const TYPE_IS_SCALAR = (type: u8): type is ScalarType => TYPE_IS_LEAF(type) && (type & BIT_SEQ) === 0;
export const TYPE_IS_SEQ = (type: u8): type is SequenceType => TYPE_IS_LEAF(type) && (type & BIT_SEQ) !== 0;

export const TYPE_IS_NONNUM = (type: u8): boolean => TYPE_IS_LEAF(type) && (type & CLASS_MASK) === CLASS_R;
export const TYPE_IS_FLOAT = (type: u8): boolean => TYPE_IS_LEAF(type) && (type & CLASS_MASK) === CLASS_F;
export const TYPE_IS_SIGNED = (type: u8): boolean => TYPE_IS_LEAF(type) && (type & CLASS_MASK) === CLASS_I;

export const I = (w: u32, sign: 1 | 0): u8 => (BIT_LEAF | ((sign ? CLASS_I : CLASS_U) | (w & ARG_MASK))) as u8;
export const F = (w: u32): u8 => (BIT_LEAF | (CLASS_F | (w & ARG_MASK))) as u8;

export const R = (x: u32): u8 => (BIT_LEAF | (CLASS_R | (x & ARG_MASK))) as u8;

export type NodeType = LeafType | SpecialType;

export type LeafType = ScalarType | SequenceType;
export const enum ScalarType {
  U8 = 0x80 | (0 | 0), // I(0, 0)
  I8 = 0x80 | (8 | 0), // I(0, 1)
  U16 = 0x80 | (0 | 1), // I(1, 0)
  I16 = 0x80 | (8 | 1), // I(1, 1)
  U32 = 0x80 | (0 | 2), // I(2, 0)
  I32 = 0x80 | (8 | 2), // I(2, 1)
  U64 = 0x80 | (0 | 3), // I(3, 0)
  I64 = 0x80 | (8 | 3), // I(3, 1)
  F32 = 0x80 | (16 | 2), // F(2)
  F64 = 0x80 | (16 | 3), // F(3)

  BOOL = 0x80 | (24 | 0), // R(0)
}

export const enum SequenceType {
  U8_ARRAY = 0x80 | 32 | (0 | 0), // BIT_SEQ | I(0, 0)
  I8_ARRAY = 0x80 | 32 | (8 | 0), // BIT_SEQ | I(0, 1)
  U16_ARRAY = 0x80 | 32 | (0 | 1), // BIT_SEQ | I(1, 0)
  I16_ARRAY = 0x80 | 32 | (8 | 1), // BIT_SEQ | I(1, 1)
  U32_ARRAY = 0x80 | 32 | (0 | 2), // BIT_SEQ | I(2, 0)
  I32_ARRAY = 0x80 | 32 | (8 | 2), // BIT_SEQ | I(2, 1)
  U64_ARRAY = 0x80 | 32 | (0 | 3), // BIT_SEQ | I(3, 0)
  I64_ARRAY = 0x80 | 32 | (8 | 3), // BIT_SEQ | I(3, 1)
  F32_ARRAY = 0x80 | 32 | (16 | 2), // BIT_SEQ | F(2)
  F64_ARRAY = 0x80 | 32 | (16 | 3), // BIT_SEQ | F(3)

  BITSET = 0x80 | 32 | (24 | 0), // BIT_SEQ | R(0)
  STR = 0x80 | 32 | (24 | 1), // BIT_SEQ | R(1)
  /** `len` + 1 u32 offsets into the UTF-8 that follows them. */
  STRS = 0x80 | 32 | (24 | 2), // BIT_SEQ | R(2)
}

export const enum SpecialType {
  NIL = 0x00, // untyped null

  BRANCH = 0x20,

  GRAFT = 0x40,
}

export type NodeValue<T = never> =
  | T
  | null
  | primitive
  | TypedArray
  | BigTypedArray
  | NodeValue<T>[];

export type Node = LeafNode | SpecialNode;

export type LeafNode = ScalarNode | SequenceNode;

export interface ScalarNode {
  readonly type: ScalarType;

  val: Nullable<numeric>;
}

export interface SequenceNode {
  readonly type: SequenceType;

  val: Nullable<string | string[] | TypedArray | BigTypedArray>;
}

export type SpecialNode = NilNode | BranchNode | GraftNode;
export interface BranchNode {
  readonly type: SpecialType.BRANCH;

  val: Nullable<Node[]>;
}

// A previously encoded packet spliced in; it must lay out as the inline subtree would.
export interface GraftNode {
  readonly type: SpecialType.GRAFT;

  val: U8Array;
}

export interface NilNode {
  readonly type: SpecialType.NIL;

  val: null;
}

export const NIL_NODE: NilNode = { type: SpecialType.NIL, val: null } as const;

export const NODE_IS_LEAF = (node: Node): node is LeafNode => TYPE_IS_LEAF(node.type);
export const NODE_IS_SCALAR = (node: Node): node is ScalarNode => TYPE_IS_SCALAR(node.type);
export const NODE_IS_SEQ = (node: Node): node is SequenceNode => TYPE_IS_SEQ(node.type);

export const NODE_IS_SPECIAL = (node: Node): node is SpecialNode => TYPE_IS_SPECIAL(node.type);
export const NODE_IS_NIL = (node: Node): node is NilNode => node.type === SpecialType.NIL;
export const NODE_IS_BRANCH = (node: Node): node is BranchNode => node.type === SpecialType.BRANCH;
export const NODE_IS_GRAFT = (node: Node): node is GraftNode => node.type === SpecialType.GRAFT;

export const NONE = 0 as const;

export const RESERVED = 0 as const;

export const LE = true as const;

export const MAGIC = 0x00505458 as const; // "XTP\0" little-endian word

export interface EncodingState {
  base: u32;
  lim: u32;
  type: NodeType;
}

export const DESC_TYPE_W: u32 = 8;
export const DESC_TYPE_SHAMT: u32 = 0;
export const DESC_TYPE_MASK: u32 = (1 << DESC_TYPE_W) - 1;

export const WORD_SZ: AlignUnit = 8;

export const SET_WORD = (buf_view: DataView, off: u32, lo32: u32, hi32: u32): void => {
  buf_view.setUint32(off, lo32, LE);
  buf_view.setUint32(off + 4, hi32, LE);
};
export const GET_WORD = (buf_view: DataView, off: u32, out: [u32, u32]): [u32, u32] => {
  out[0] = buf_view.getUint32(off, LE);
  out[1] = buf_view.getUint32(off + 4, LE);
  return out;
};

export type AlignUnit = 1 | 2 | 4 | 8;

export const ALIGN = (nbyte: u32, unit: AlignUnit): u32 => {
  if ((unit & (unit - 1)) || unit > WORD_SZ) throw new MarshalError(Errno.EINVAL);

  const mask = unit - 1;
  // `&` works in int32: past 2^31 the aligned size came out negative.
  if (nbyte > 0xffffffff - mask) throw new MarshalError(Errno.EOVERFLOW);
  return ((nbyte + mask) & ~mask) >>> 0;
};

export const ALIGN_SZ = (type: u8): AlignUnit => {
  if (type === SpecialType.BRANCH || TYPE_IS_SEQ(type)) return WORD_SZ;

  switch (type) {
    case ScalarType.U8:
    case ScalarType.I8:
      return 1;
    case ScalarType.U16:
    case ScalarType.I16:
      return 2;
    case ScalarType.U32:
    case ScalarType.I32:
      return 4;
    case ScalarType.U64:
    case ScalarType.I64:
      return 8;
    case ScalarType.F32:
      return 4;
    case ScalarType.F64:
      return 8;

    case ScalarType.BOOL:
      return 1;

    default:
      throw new MarshalError(Errno.EBADMSG);
  }
};

export const HDR_SZ: u32 = WORD_SZ * 2; // must stay a multiple of WORD_SZ

export const MAX_PKT_SZ = 0x7fffffff;
