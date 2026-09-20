// @xpute/core/wire/xtp/view.ts

import type { f32, f64, i16, i32, i64, i8, u16, u32, u64, u8 } from "@xpute/core/abi/word.ts";
import { BOOL, F32, F64, I16, I32, I64, I8, U16, U32, U64, U8 } from "@xpute/core/abi/word.ts";
import { fits_i32, fits_i64 } from "@xpute/core/abi/word.ts";
import type { F32Array, F64Array, I16Array, I32Array, I64Array, I8Array, U16Array, U32Array, U64Array, U8Array } from "@xpute/core/abi/array.ts";
import { Errno } from "@xpute/core/status/errno.spec.ts";
import { MarshalError } from "@xpute/core/status/error.ts";

import type { BranchNode, Node, NodeValue } from "./spec.ts";
import { NIL_NODE, ScalarType, SequenceType, SpecialType, WORD_SZ } from "./spec.ts";

const U8_NODE = (u: u8 | null): Node => ({ type: ScalarType.U8, val: u === null ? null : U8(u) });
const I8_NODE = (i: i8 | null): Node => ({ type: ScalarType.I8, val: i === null ? null : I8(i) });
const U16_NODE = (u: u16 | null): Node => ({ type: ScalarType.U16, val: u === null ? null : U16(u) });
const I16_NODE = (i: i16 | null): Node => ({ type: ScalarType.I16, val: i === null ? null : I16(i) });
const U32_NODE = (u: u32 | null): Node => ({ type: ScalarType.U32, val: u === null ? null : U32(u) });
const I32_NODE = (i: i32 | null): Node => ({ type: ScalarType.I32, val: i === null ? null : I32(i) });
const U64_NODE = (u: u64 | null): Node => ({ type: ScalarType.U64, val: u === null ? null : U64(u) });
const I64_NODE = (i: i64 | null): Node => ({ type: ScalarType.I64, val: i === null ? null : I64(i) });
const F32_NODE = (f: f32 | null): Node => ({ type: ScalarType.F32, val: f === null ? null : F32(f) });
const F64_NODE = (f: f64 | null): Node => ({ type: ScalarType.F64, val: f === null ? null : F64(f) });

const BOOL_NODE = (b: boolean | null): Node => ({ type: ScalarType.BOOL, val: b === null ? null : BOOL(b) });

const U8_ARRAY_NODE = (arr: U8Array | null): Node => ({ type: SequenceType.U8_ARRAY, val: arr });
const I8_ARRAY_NODE = (arr: I8Array | null): Node => ({ type: SequenceType.I8_ARRAY, val: arr });
const U16_ARRAY_NODE = (arr: U16Array | null): Node => ({ type: SequenceType.U16_ARRAY, val: arr });
const I16_ARRAY_NODE = (arr: I16Array | null): Node => ({ type: SequenceType.I16_ARRAY, val: arr });
const U32_ARRAY_NODE = (arr: U32Array | null): Node => ({ type: SequenceType.U32_ARRAY, val: arr });
const I32_ARRAY_NODE = (arr: I32Array | null): Node => ({ type: SequenceType.I32_ARRAY, val: arr });
const U64_ARRAY_NODE = (arr: U64Array | null): Node => ({ type: SequenceType.U64_ARRAY, val: arr });
const I64_ARRAY_NODE = (arr: I64Array | null): Node => ({ type: SequenceType.I64_ARRAY, val: arr });
const F32_ARRAY_NODE = (arr: F32Array | null): Node => ({ type: SequenceType.F32_ARRAY, val: arr });
const F64_ARRAY_NODE = (arr: F64Array | null): Node => ({ type: SequenceType.F64_ARRAY, val: arr });

const BITSET_NODE = (arr: U8Array | null): Node => ({ type: SequenceType.BITSET, val: arr });
const STR_NODE = (s: string | null): Node => ({ type: SequenceType.STR, val: s });

function val_to_node(val: NodeValue<NodeView>): Node {
  if (val instanceof NodeView) return (val.node);

  if (val === null) return NIL_NODE;

  if (typeof val === "number") {
    if (fits_i32(val)) return I32_NODE(val);
    return F64_NODE(val);
  }
  if (typeof val === "bigint") {
    if (!fits_i64(val)) throw new MarshalError(Errno.EINVAL, "bigint does not fit i64");
    return I64_NODE(val);
  }
  if (typeof val === "boolean") return BOOL_NODE(val);
  if (typeof val === "string") return STR_NODE(val);

  if (val instanceof Uint8Array) return U8_ARRAY_NODE(val);
  if (val instanceof Int8Array) return I8_ARRAY_NODE(val);
  if (val instanceof Uint16Array) return U16_ARRAY_NODE(val);
  if (val instanceof Int16Array) return I16_ARRAY_NODE(val);
  if (val instanceof Uint32Array) return U32_ARRAY_NODE(val);
  if (val instanceof Int32Array) return I32_ARRAY_NODE(val);
  if (val instanceof Float32Array) return F32_ARRAY_NODE(val);
  if (val instanceof Float64Array) return F64_ARRAY_NODE(val);
  if (val instanceof BigUint64Array) return U64_ARRAY_NODE(val);
  if (val instanceof BigInt64Array) return I64_ARRAY_NODE(val);

  if (Array.isArray(val)) {
    const branch = new BranchViewImpl();
    for (const child of val) branch.put(child);
    return (branch.node);
  }

  throw new MarshalError(Errno.EINVAL, `unsupported value: ${val}`);
}

// ============ View ============

export abstract class NodeView {
  abstract node: Node;

  abstract nil(): this;

  // ---- Scalar ----
  // explicit scalar writers are caller-chosen constructors;
  // inputs are normalized through the corresponding word cast helpers.
  // typed optional leaf writers preserve node kind;
  // physical presence is decided during encode.

  abstract u8(u: u8 | null): this;
  abstract i8(i: i8 | null): this;
  abstract u16(u: u16 | null): this;
  abstract i16(i: i16 | null): this;
  abstract u32(u: u32 | null): this;
  abstract i32(i: i32 | null): this;
  abstract u64(u: u64 | null): this;
  abstract i64(i: i64 | null): this;
  abstract f32(f: f32 | null): this;
  abstract f64(f: f64 | null): this;

  abstract bool(b: boolean | null): this;

  // ---- Sequence ----

  abstract u8_array(arr: U8Array | null): this;
  abstract i8_array(arr: I8Array | null): this;
  abstract u16_array(arr: U16Array | null): this;
  abstract i16_array(arr: I16Array | null): this;
  abstract u32_array(arr: U32Array | null): this;
  abstract i32_array(arr: I32Array | null): this;
  abstract u64_array(arr: U64Array | null): this;
  abstract i64_array(arr: I64Array | null): this;
  abstract f32_array(arr: F32Array | null): this;
  abstract f64_array(arr: F64Array | null): this;

  abstract bitset(arr: U8Array | null): this;

  abstract str(s: string | null): this;

  // ---- Subtree ----

  /** Builds a branch subtree inline and writes it into the current view. */
  abstract branch(fn: ((b: BranchView) => void) | null): this;

  /**
   * Grafts an already-encoded subtree packet.
   *
   * Contract:
   * - `pkt` must be a tree packet carrier intended for this format
   * - `pkt.byteOffset` must be WORD_SZ-aligned
   * - The grafted root may be physically null
   * - The caller must not mutate `pkt` after grafting it
   *
   * This method enforces only construction-boundary checks.
   * Packet header validation remains a read/encode-side concern.
   */
  abstract graft(pkt: U8Array): this;
}

export class TreeView extends NodeView {
  override node: Node = NIL_NODE;

  // ---- Write Primitive ----

  /** Replaces the current root node. */
  protected _set_node(node: Node): this {
    this.node = node;
    return this;
  }

  /**
   * Lowers a generic JS value into the current root node.
   *
   * Mapping:
   * - number  -> i32 if it fits_i32(), otherwise f64
   * - bigint  -> i64
   * - boolean -> bool
   * - string  -> str
   * - TypedArray / BigTypedArray -> matching array node
   * - Array -> branch preserving element order
   *
   * Note:
   * - NodeValue currently excludes plain objects.
   * - key-bearing object semantics must be modeled by a higher-level schema/plugin.
   * - The number -> i32/f64 split is a JS-side lowering policy, not a wire-level
   *   requirement of the tree packet format itself.
   *
   * Contract:
   * - Unsupported input is treated as programmer error and throws.
   * - bigint is accepted only when it fits the i64 lowering path.
   * - Generic scalar bigint lowering currently targets only i64.
   *   Unsigned 64-bit scalar writes must use the explicit u64() writer.
   * - Explicit writers such as u8(), i64(), f64() are considered caller-chosen
   *   leaf constructors and therefore do not add generic lowering validation.
   */
  set<T extends NodeValue<NodeView>>(val: T): this {
    return this._set_node(val_to_node(val));
  }

  override nil(): this {
    // generic/untyped null lowering
    return this._set_node(NIL_NODE);
  }

  // ---- Scalar ----
  // explicit scalar writers are caller-chosen constructors;
  // inputs are normalized through the corresponding word cast helpers.
  // typed optional leaf writers preserve node kind;
  // physical presence is decided during encode.

  override u8(u: u8 | null): this {
    return this._set_node(U8_NODE(u));
  }
  override i8(i: i8 | null): this {
    return this._set_node(I8_NODE(i));
  }
  override u16(u: u16 | null): this {
    return this._set_node(U16_NODE(u));
  }
  override i16(i: i16 | null): this {
    return this._set_node(I16_NODE(i));
  }
  override u32(u: u32 | null): this {
    return this._set_node(U32_NODE(u));
  }
  override i32(i: i32 | null): this {
    return this._set_node(I32_NODE(i));
  }
  override u64(u: u64 | null): this {
    return this._set_node(U64_NODE(u));
  }
  override i64(i: i64 | null): this {
    return this._set_node(I64_NODE(i));
  }
  override f32(f: f32 | null): this {
    return this._set_node(F32_NODE(f));
  }
  override f64(f: f64 | null): this {
    return this._set_node(F64_NODE(f));
  }

  override bool(b: boolean | null): this {
    return this._set_node(BOOL_NODE(b));
  }

  // ---- Sequence ----

  override u8_array(arr: U8Array | null): this {
    return this._set_node(U8_ARRAY_NODE(arr));
  }
  override i8_array(arr: I8Array | null): this {
    return this._set_node(I8_ARRAY_NODE(arr));
  }
  override u16_array(arr: U16Array | null): this {
    return this._set_node(U16_ARRAY_NODE(arr));
  }
  override i16_array(arr: I16Array | null): this {
    return this._set_node(I16_ARRAY_NODE(arr));
  }
  override u32_array(arr: U32Array | null): this {
    return this._set_node(U32_ARRAY_NODE(arr));
  }
  override i32_array(arr: I32Array | null): this {
    return this._set_node(I32_ARRAY_NODE(arr));
  }
  override u64_array(arr: U64Array | null): this {
    return this._set_node(U64_ARRAY_NODE(arr));
  }
  override i64_array(arr: I64Array | null): this {
    return this._set_node(I64_ARRAY_NODE(arr));
  }
  override f32_array(arr: F32Array | null): this {
    return this._set_node(F32_ARRAY_NODE(arr));
  }
  override f64_array(arr: F64Array | null): this {
    return this._set_node(F64_ARRAY_NODE(arr));
  }

  override bitset(arr: U8Array | null): this {
    return this._set_node(BITSET_NODE(arr));
  }

  override str(s: string | null): this {
    return this._set_node(STR_NODE(s));
  }

  // ---- Subtree ----

  /** Builds a branch subtree inline and writes it into the current root node. */
  override branch(fn: ((b: BranchView) => void) | null): this {
    // typed optional branch; node kind is preserved and physical presence is decided during encode
    if (fn === null) return this._set_node({ type: SpecialType.BRANCH, val: null });

    const branch = new BranchViewImpl();
    fn(branch);
    return this._set_node(branch.node);
  }

  /**
   * Grafts an already-encoded subtree packet into the current root node.
   *
   * Contract:
   * - `pkt.byteOffset` must be WORD_SZ-aligned
   * - The grafted root may be physically null
   * - The caller must not mutate `pkt` after grafting it
   *
   * This method enforces only construction-boundary checks.
   * Full packet validation is intentionally out of scope here.
   */
  override graft(pkt: U8Array): this {
    if (pkt.byteOffset % WORD_SZ) throw new MarshalError(Errno.EBADMSG, `bad graft packet: misaligned base offset ${pkt.byteOffset}`);
    return this._set_node({ type: SpecialType.GRAFT, val: pkt });
  }
}

export abstract class BranchView extends NodeView {
  override node: BranchNode = { type: SpecialType.BRANCH, val: [] };

  // ---- Append Primitive ----

  /** Replaces all existing children with the provided sequence. */
  set<T extends NodeValue<NodeView>[]>(vals: T): this {
    this.node.val!.length = 0;
    for (const val of vals) this.put(val);
    return this;
  }

  /** Appends one child node to the current branch. */
  protected _put_node(node: Node): this {
    this.node.val!.push(node);
    return this;
  }

  /** Lowers one generic JS value and appends it as a child. */
  put<T extends NodeValue<NodeView>>(val: T): this {
    return this._put_node(val_to_node(val));
  }

  override nil(): this {
    // generic/untyped null lowering
    return this._put_node(NIL_NODE);
  }

  // ---- scalar ----

  override u8(u: u8 | null): this {
    return this._put_node(U8_NODE(u));
  }
  override i8(i: i8 | null): this {
    return this._put_node(I8_NODE(i));
  }
  override u16(u: u16 | null): this {
    return this._put_node(U16_NODE(u));
  }
  override i16(i: i16 | null): this {
    return this._put_node(I16_NODE(i));
  }
  override u32(u: u32 | null): this {
    return this._put_node(U32_NODE(u));
  }
  override i32(i: i32 | null): this {
    return this._put_node(I32_NODE(i));
  }
  override u64(u: u64 | null): this {
    return this._put_node(U64_NODE(u));
  }
  override i64(i: i64 | null): this {
    return this._put_node(I64_NODE(i));
  }
  override f32(f: f32 | null): this {
    return this._put_node(F32_NODE(f));
  }
  override f64(f: f64 | null): this {
    return this._put_node(F64_NODE(f));
  }

  override bool(b: boolean | null): this {
    return this._put_node(BOOL_NODE(b));
  }

  // ---- sequence ----

  override u8_array(arr: U8Array | null): this {
    return this._put_node(U8_ARRAY_NODE(arr));
  }
  override i8_array(arr: I8Array | null): this {
    return this._put_node(I8_ARRAY_NODE(arr));
  }
  override u16_array(arr: U16Array | null): this {
    return this._put_node(U16_ARRAY_NODE(arr));
  }
  override i16_array(arr: I16Array | null): this {
    return this._put_node(I16_ARRAY_NODE(arr));
  }
  override u32_array(arr: U32Array | null): this {
    return this._put_node(U32_ARRAY_NODE(arr));
  }
  override i32_array(arr: I32Array | null): this {
    return this._put_node(I32_ARRAY_NODE(arr));
  }
  override u64_array(arr: U64Array | null): this {
    return this._put_node(U64_ARRAY_NODE(arr));
  }
  override i64_array(arr: I64Array | null): this {
    return this._put_node(I64_ARRAY_NODE(arr));
  }
  override f32_array(arr: F32Array | null): this {
    return this._put_node(F32_ARRAY_NODE(arr));
  }
  override f64_array(arr: F64Array | null): this {
    return this._put_node(F64_ARRAY_NODE(arr));
  }

  override bitset(arr: U8Array | null): this {
    return this._put_node(BITSET_NODE(arr));
  }

  override str(s: string | null): this {
    return this._put_node(STR_NODE(s));
  }

  // ---- Subtree ----

  override branch(fn: ((b: BranchView) => void) | null): this {
    // typed optional branch; node kind is preserved and physical presence is decided during encode
    if (fn === null) return this._put_node({ type: SpecialType.BRANCH, val: null });

    const branch = new BranchViewImpl();
    fn(branch);
    return this._put_node(branch.node);
  }

  /**
   * Appends an in-memory subtree as one child.
   * The subtree is encoded inline as part of the current tree.
   */
  subtree(subtree: NodeView): this {
    return this._put_node(subtree.node);
  }

  /**
   * Grafts an already-encoded subtree packet as one child.
   *
   * Contract:
   * - `pkt.byteOffset` must be WORD_SZ-aligned
   * - Full packet validation is intentionally deferred
   */
  override graft(pkt: U8Array): this {
    if (pkt.byteOffset % WORD_SZ) throw new MarshalError(Errno.EBADMSG, `bad graft packet: misaligned base offset ${pkt.byteOffset}`);
    return this._put_node({ type: SpecialType.GRAFT, val: pkt });
  }
}
class BranchViewImpl extends BranchView {}
