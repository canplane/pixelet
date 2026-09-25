// @xpute/core/wire/xtp/spec.ts

/**
 * Relocatable tree packet format.
 *
 * Scope
 * -----
 * - Defines a relocatable packet format for tree-shaped payloads.
 * - Each packet contains a fixed-size header and one encoded root subtree payload.
 * - Internal subtree references are position-independent through relative offsets.
 *
 * Model
 * -----
 * - Scalar leaves are headerless payloads.
 * - Sequence leaves are encoded as [len slot | payload], with payload starting
 *   immediately after the len slot.
 * - Sequence payload alignment is derived from the WORD-aligned node base.
 * - Branch nodes are encoded as [len slot | offset table].
 * - Graft nodes splice a previously encoded subtree packet.
 * - Physical null at the packet root is encoded by preserving the node type and
 *   writing root_payload_sz = 0.
 * - Physical null at a branch child is encoded by preserving the child type and
 *   writing child_rel_off = 0.
 * - Generic null lowering uses the NIL special node.
 *
 * Semantics
 * ---------
 * - This is a relocatable tree packet format, not a self-delimiting stream format.
 * - The format itself does not define schema semantics.
 * - Higher layers define field meaning, ordering rules, and validation policy.
 * - Graft is a splice primitive, not an independent payload class.
 * - A grafted subtree must produce the same root payload layout as if encoded inline.
 * - Typed optional values preserve their original node kind even when physically absent.
 * - Therefore `str(null)`, `u32(null)`, `branch(null)`, and generic `set(null)` / `put(null)` are distinct lowerings.
 *
 * Design Goals
 * ------------
 * - Relocatable tree packet layout
 * - Lazy cursor-style traversal
 * - WORD-aligned structural-node discipline
 * - sequence payload alignment derived from fixed [len slot | payload] layout
 * - JS-friendly low-32-bit len/off/size semantics in 8-byte slots
 *
 * Optional model:
 * - packet root physical absence = root_payload_sz == 0 with preserved node type
 * - branch child physical absence = child_rel_off == 0 with preserved child type
 * - generic set(null) / put(null) lower to NIL
 * - typed writers preserve their original node kind
 * - NullCursor preserves type metadata, while get() reconstructs null and opt() returns undefined
 */

// Format contracts:
// - wire byte order is little-endian for all multi-byte scalar values and len/off/size word fields
// - all len/off fields occupy one 8-byte slot
// - len/off/size semantics currently use only the low 32 bits
// - high 32 bits are reserved for future use
// - branch and sequence nodes are aligned to at least WORD_SZ
// - final packet size is aligned to WORD_SZ
// - every node type is exactly 1 byte wide in the format
// - the header type field occupies bits 0..7 of a 32-bit slot
// - bits 8..31 of that slot are reserved
//
// Packet header:
// - packet header is two 8-byte words:
//   - word0: [magic 32 | reserved 32]
//   - word1: [root_payload_sz 32 | type 8 | reserved 24]
// - HDR_SZ is required to be aligned to WORD_SZ
// - therefore a present root payload always starts exactly at HDR_SZ
// - root_payload_sz is the byte size of the encoded root payload region,
//   excluding the fixed packet header
// - root_payload_sz is constrained to:
//   - 0                    : physical null root
//   - 1..(packet_size-HDR_SZ) : present root payload
// - type is always preserved even when root_payload_sz == 0
// - therefore a physically null root may still carry typed metadata
//
// Branch child entry:
// - each child entry is one 8-byte word pair:
//   - lo32: child_rel_off
//   - hi32: type in bits 0..7, remaining bits reserved
// - child_rel_off is relative to the enclosing branch base
// - child_rel_off == 0 means physical null while preserving child type
// - child_rel_off != 0 must satisfy:
//   - child_base = branch_base + child_rel_off
//   - child_base >= table_end
//   - child_base < packet_size
// - child_rel_off must not point into:
//   - the branch header (base .. base + WORD_SZ)
//   - the offset table (base + WORD_SZ .. table_end)
// - any violation is a malformed packet

import type { numeric, primitive, u32, u8 } from "@xpute/core/abi/word.ts";
import type { BigTypedArray, TypedArray, U8Array } from "@xpute/core/abi/array.ts";
import type { Nullable } from "@xpute/core/kit/type.ts";
import { Errno } from "@xpute/core/status/errno.spec.ts";
import { MarshalError } from "@xpute/core/status/error.ts";

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

const ARG_W = 3;
const CLASS_W = 2;

// positions
const ARG_SHAMT: u8 = 0;
const CLASS_SHAMT: u8 = ARG_SHAMT + ARG_W; // bits 3..4
const SEQ_SHAMT: u8 = CLASS_SHAMT + CLASS_W; // bit 5
const LEAF_SHAMT: u8 = 7; // bit 7

// widths & masks
export const ARG_MASK: u8 = ((1 << ARG_W) - 1) as u8;
const CLASS_MASK: u8 = (((1 << CLASS_W) - 1) << CLASS_SHAMT) as u8;

// node bits
const BIT_SEQ: u8 = (1 << SEQ_SHAMT) as u8;
const BIT_LEAF: u8 = (1 << LEAF_SHAMT) as u8;

// leaf class tags
const CLASS_U: u8 = (0 << CLASS_SHAMT) as u8;
const CLASS_I: u8 = (1 << CLASS_SHAMT) as u8;
const CLASS_F: u8 = (2 << CLASS_SHAMT) as u8;
const CLASS_R: u8 = (3 << CLASS_SHAMT) as u8;

// ---- Extract Helpers ----

export const TYPE_ARG_OF = (type: u8): u8 => (type & ARG_MASK) as u8;

export const TYPE_IS_LEAF = (type: u8): type is LeafType => (type & BIT_LEAF) !== 0;
export const TYPE_IS_SPECIAL = (type: u8): type is SpecialType => (type & BIT_LEAF) === 0;

export const TYPE_IS_SCALAR = (type: u8): type is ScalarType => TYPE_IS_LEAF(type) && (type & BIT_SEQ) === 0;
export const TYPE_IS_SEQ = (type: u8): type is SequenceType => TYPE_IS_LEAF(type) && (type & BIT_SEQ) !== 0;

export const TYPE_IS_NONNUM = (type: u8): boolean => TYPE_IS_LEAF(type) && (type & CLASS_MASK) === CLASS_R;
export const TYPE_IS_FLOAT = (type: u8): boolean => TYPE_IS_LEAF(type) && (type & CLASS_MASK) === CLASS_F;
export const TYPE_IS_SIGNED = (type: u8): boolean => TYPE_IS_LEAF(type) && (type & CLASS_MASK) === CLASS_I;

// ---- Type Constructors ----

// numeric pack:
export const I = (w: u32, sign: 1 | 0): u8 => (BIT_LEAF | ((sign ? CLASS_I : CLASS_U) | (w & ARG_MASK))) as u8;
export const F = (w: u32): u8 => (BIT_LEAF | (CLASS_F | (w & ARG_MASK))) as u8;

// non-numeric pack:
export const R = (x: u32): u8 => (BIT_LEAF | (CLASS_R | (x & ARG_MASK))) as u8;

// ---- Type Tables ----

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
}

// special type space (when L = 0)
export const enum SpecialType {
  NIL = 0x00, // generic/untyped null node

  // structural node
  BRANCH = 0x20, // 0010 0000

  // subtree splice primitive
  GRAFT = 0x40, // 0100 0000
}

// ============ Model ============

// generic input/output value domain used by tree views and cursors
// plain object lowering is intentionally excluded for now
export type NodeValue<T = never> =
  | T
  | null
  | primitive
  | TypedArray
  | BigTypedArray
  | NodeValue<T>[];

export type Node = LeafNode | SpecialNode;

// ---- Leaf Nodes ----

export type LeafNode = ScalarNode | SequenceNode;

export interface ScalarNode {
  readonly type: ScalarType;

  val: Nullable<numeric>;
}

export interface SequenceNode {
  readonly type: SequenceType;

  val: Nullable<string | TypedArray | BigTypedArray>;
}

// ---- Special Nodes ----

export type SpecialNode = NilNode | BranchNode | GraftNode;
export interface BranchNode {
  readonly type: SpecialType.BRANCH;

  val: Nullable<Node[]>;
}

// special splice primitive for a previously encoded subtree packet
export interface GraftNode {
  readonly type: SpecialType.GRAFT;

  val: U8Array;
}

// ---- Optional ----

// Optional encoding model:
// - optional means the node kind is preserved while payload is absent
// - packet-root physical absence is encoded as root_payload_sz == 0
// - branch-child physical absence is encoded as child_rel_off == 0
// - generic set(null) / put(null) lower to NIL
// - typed writers preserve their original node kind
export interface NilNode {
  readonly type: SpecialType.NIL;

  val: null;
}

export const NIL_NODE: NilNode = { type: SpecialType.NIL, val: null } as const;

// ---- Predicates ----

// export const NODE_IS_NOT_NULL = (node: Optional<Node>): node is Node => node.val !== null;

export const NODE_IS_LEAF = (node: Node): node is LeafNode => TYPE_IS_LEAF(node.type);
export const NODE_IS_SCALAR = (node: Node): node is ScalarNode => TYPE_IS_SCALAR(node.type);
export const NODE_IS_SEQ = (node: Node): node is SequenceNode => TYPE_IS_SEQ(node.type);

export const NODE_IS_SPECIAL = (node: Node): node is SpecialNode => TYPE_IS_SPECIAL(node.type);
export const NODE_IS_NIL = (node: Node): node is NilNode => node.type === SpecialType.NIL;
export const NODE_IS_BRANCH = (node: Node): node is BranchNode => node.type === SpecialType.BRANCH;
export const NODE_IS_GRAFT = (node: Node): node is GraftNode => node.type === SpecialType.GRAFT;

// ============ Wire Primitives ============

// ---- Sentinels ----

export const NONE = 0 as const; // reserved zero on wire; reused as local uninitialized sentinel

export const RESERVED = 0 as const;

// ---- Endianness / Magic ----

export const LE = true as const;

export const MAGIC = 0x00505458 as const; // "XTP\0" little-endian word

// ---- Descriptor Layout ----

// encoder work state for one node; updated progressively during encode
export interface EncodingState {
  base: u32;
  lim: u32;
  type: NodeType;
}

// NONE is reused as the local uninitialized sentinel during encode.
export const DESC_TYPE_W: u32 = 8;
export const DESC_TYPE_SHAMT: u32 = 0;
export const DESC_TYPE_MASK: u32 = (1 << DESC_TYPE_W) - 1;

// ---- Word Helpers ----

export const WORD_SZ: AlignUnit = 8; // 8 bytes

export const SET_WORD = (buf_view: DataView, off: u32, lo32: u32, hi32: u32): void => {
  buf_view.setUint32(off, lo32, LE);
  buf_view.setUint32(off + 4, hi32, LE);
};
export const GET_WORD = (buf_view: DataView, off: u32, out: [u32, u32]): [u32, u32] => {
  out[0] = buf_view.getUint32(off, LE); // lo
  out[1] = buf_view.getUint32(off + 4, LE); // hi
  return out;
};

// ============ Layout ============

// ---- Alignment ----

export type AlignUnit = 1 | 2 | 4 | 8;

/**
 * Rounds `nbyte` up to the next multiple of `unit`.
 *
 * Requirements:
 * - `unit` must be non-zero
 * - `unit` must be a power of two
 * - `unit` must not exceed WORD_SZ
 */
export const ALIGN = (nbyte: u32, unit: AlignUnit): u32 => {
  if ((unit & (unit - 1)) || unit > WORD_SZ) throw new MarshalError(Errno.EINVAL);

  const mask = unit - 1;
  // `&` works in int32: past 2^31 the aligned size came out negative.
  if (nbyte > 0xffffffff - mask) throw new MarshalError(Errno.EOVERFLOW);
  return ((nbyte + mask) & ~mask) >>> 0;
};

// ---- Node Layout ----

export const ALIGN_SZ = (type: u8): AlignUnit => {
  // alignment rules (scalar leaf, sequence leaf, or branch only):
  //
  // scalar:
  //   align to element size
  //
  // sequence / branch:
  //   align to WORD_SZ so the leading len slot is naturally aligned
  //   and child tables remain word-aligned
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

// ---- Packet Layout ----

export const HDR_SZ: u32 = WORD_SZ * 2; // 16 bytes, must remain aligned to WORD_SZ

export const MAX_PKT_SZ = 0x7fffffff; // 2GB - 1
