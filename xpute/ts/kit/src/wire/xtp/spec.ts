// @xpute/kit/wire/xtp/spec.ts

// Relocatable tree packet: a 16-byte header and one root subtree, with children
// addressed by offsets relative to their branch, so a packet can be grafted
// anywhere unchanged. Little-endian; every len/off/size field is an 8-byte slot
// of which only the low 32 bits are used.
//
// Header: word0 = [magic 32 | reserved 32], word1 = [root_payload_sz 32 | type 8 | reserved 24].
// Branch: [len slot | child table], each entry [child_rel_off 32 | type 8 | reserved 24].
// A typed null keeps its type: root_payload_sz == 0 at the root, child_rel_off == 0
// in a branch. A nonzero child_rel_off lands past the table and before the next
// present sibling, inside its parent's extent, or the packet is malformed.

import type { numeric, primitive, u32, u8 } from "@xpute/kit/abi/word.ts";
import { FIELD_GET, FIELD_SET } from "@xpute/kit/abi/word.ts";
import type {
  BigTypedArray,
  TypedArray,
  U8Array,
} from "@xpute/kit/abi/array.ts";
import * as Errno from "@xpute/kit/status/errno.spec.ts";
import { MarshalError } from "@xpute/kit/status/error.ts";
import * as ScalarType from "./scalar_type.ts";
import * as SequenceType from "./sequence_type.ts";
import * as SpecialType from "./special_type.ts";

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

export const TYPE_IS_LEAF = (type: u8): boolean => (type & BIT_LEAF) !== 0;
export const TYPE_IS_SPECIAL = (type: u8): boolean => (type & BIT_LEAF) === 0;

export const TYPE_IS_SCALAR = (type: u8): boolean =>
  TYPE_IS_LEAF(type) && (type & BIT_SEQ) === 0;
export const TYPE_IS_SEQ = (type: u8): boolean =>
  TYPE_IS_LEAF(type) && (type & BIT_SEQ) !== 0;

export const TYPE_IS_NONNUM = (type: u8): boolean =>
  TYPE_IS_LEAF(type) && (type & CLASS_MASK) === CLASS_R;
export const TYPE_IS_FLOAT = (type: u8): boolean =>
  TYPE_IS_LEAF(type) && (type & CLASS_MASK) === CLASS_F;
export const TYPE_IS_SIGNED = (type: u8): boolean =>
  TYPE_IS_LEAF(type) && (type & CLASS_MASK) === CLASS_I;

export const I = (w: u32, sign: 1 | 0): u8 =>
  (BIT_LEAF | ((sign ? CLASS_I : CLASS_U) | (w & ARG_MASK))) as u8;
export const F = (w: u32): u8 => (BIT_LEAF | (CLASS_F | (w & ARG_MASK))) as u8;

export const R = (x: u32): u8 => (BIT_LEAF | (CLASS_R | (x & ARG_MASK))) as u8;

export type NodeType = u8;

export type LeafType = u8;
/** One of ScalarType's, as its enum was a type. */
type ScalarType = (typeof ScalarType)[keyof typeof ScalarType];
export { ScalarType };

/** One of SequenceType's, as its enum was a type. */
type SequenceType = (typeof SequenceType)[keyof typeof SequenceType];
export { SequenceType };

/** One of SpecialType's, as its enum was a type. */
type SpecialType = (typeof SpecialType)[keyof typeof SpecialType];
export { SpecialType };

export type NodeValue<T = never> =
  T | null | primitive | TypedArray | BigTypedArray | NodeValue<T>[];

export type Node = LeafNode | SpecialNode;

export type LeafNode = ScalarNode | SequenceNode;

export interface ScalarNode {
  readonly type: ScalarType;

  val: numeric | null;
}

export interface SequenceNode {
  readonly type: SequenceType;

  val: string | string[] | TypedArray | BigTypedArray | null;
}

export type SpecialNode = NilNode | BranchNode | GraftNode;
export interface BranchNode {
  readonly type: typeof SpecialType.BRANCH;

  val: Node[] | null;
}

// A previously encoded packet spliced in; it must lay out as the inline subtree would.
export interface GraftNode {
  readonly type: typeof SpecialType.GRAFT;

  val: U8Array;
}

export interface NilNode {
  readonly type: typeof SpecialType.NIL;

  val: null;
}

export const NIL_NODE: NilNode = { type: SpecialType.NIL, val: null } as const;

export const NODE_IS_LEAF = (node: Node): node is LeafNode =>
  TYPE_IS_LEAF(node.type);
export const NODE_IS_SCALAR = (node: Node): node is ScalarNode =>
  TYPE_IS_SCALAR(node.type);
export const NODE_IS_SEQ = (node: Node): node is SequenceNode =>
  TYPE_IS_SEQ(node.type);

export const NODE_IS_SPECIAL = (node: Node): node is SpecialNode =>
  TYPE_IS_SPECIAL(node.type);
export const NODE_IS_NIL = (node: Node): node is NilNode =>
  node.type === SpecialType.NIL;
export const NODE_IS_BRANCH = (node: Node): node is BranchNode =>
  node.type === SpecialType.BRANCH;
export const NODE_IS_GRAFT = (node: Node): node is GraftNode =>
  node.type === SpecialType.GRAFT;

export const NONE = 0 as const;

export const RESERVED = 0 as const;

export const LE = true as const;

export const MAGIC = 0x00505458 as const; // "XTP\0" little-endian word

export const DESC_TYPE_W: u32 = 8;
export const DESC_TYPE_SHAMT: u32 = 0;
export const DESC_TYPE_MASK: u32 = (1 << DESC_TYPE_W) - 1;

export const WORD_SZ: AlignUnit = 8;

export const SET_WORD = (
  buf_view: DataView,
  off: u32,
  lo32: u32,
  hi32: u32,
): void => {
  buf_view.setUint32(off, lo32, LE);
  buf_view.setUint32(off + 4, hi32, LE);
};
export const GET_WORD = (
  buf_view: DataView,
  off: u32,
  out: [u32, u32],
): [u32, u32] => {
  out[0] = buf_view.getUint32(off, LE);
  out[1] = buf_view.getUint32(off + 4, LE);
  return out;
};

/** A type the wire carries: the untyped null, a branch, or a leaf of a width its class has. */
export const TYPE_IS_KNOWN = (type: u8): boolean => {
  if (type === SpecialType.NIL || type === SpecialType.BRANCH) return true;
  if (
    !TYPE_IS_LEAF(type) ||
    type & ~(BIT_LEAF | BIT_SEQ | CLASS_MASK | ARG_MASK)
  )
    return false;
  const arg = TYPE_ARG_OF(type);
  switch (type & CLASS_MASK) {
    case CLASS_F:
      return arg === 2 || arg === 3;
    case CLASS_R:
      return arg <= (TYPE_IS_SEQ(type) ? 2 : 0);
    default:
      return arg <= 3;
  }
};

/** A read slot whose high half is reserved, zero on the wire: a len, a size, the magic. */
export const GET_LEN = (buf_view: DataView, off: u32): u32 => {
  if (buf_view.getUint32(off + 4, LE) !== RESERVED) {
    throw new MarshalError(Errno.EBADMSG);
  }
  return buf_view.getUint32(off, LE);
};

/** A read child entry or root word: its offset or size, and a known type with the rest reserved. */
export const GET_DESC = (
  buf_view: DataView,
  off: u32,
  out: [u32, NodeType],
): [u32, NodeType] => {
  const desc = buf_view.getUint32(off + 4, LE);
  const type = FIELD_GET(desc, DESC_TYPE_SHAMT, DESC_TYPE_MASK) as NodeType;
  if (
    desc !== FIELD_SET(0, DESC_TYPE_SHAMT, DESC_TYPE_MASK, type) ||
    !TYPE_IS_KNOWN(type)
  )
    throw new MarshalError(Errno.EBADMSG);
  out[0] = buf_view.getUint32(off, LE);
  out[1] = type;
  return out;
};

export type AlignUnit = 1 | 2 | 4 | 8;

export const ALIGN = (nbyte: u32, unit: AlignUnit): u32 => {
  if (unit & (unit - 1) || unit > WORD_SZ) {
    throw new MarshalError(Errno.EINVAL);
  }

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
