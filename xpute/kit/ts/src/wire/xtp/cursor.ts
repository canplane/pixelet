// @xpute/kit/wire/xtp/cursor.ts

import type {
  f32,
  f64,
  i16,
  i32,
  i64,
  i8,
  u16,
  u32,
  u64,
  u8,
} from "@xpute/kit/abi/word.ts";
import { BOOL } from "@xpute/kit/abi/word.ts";
import type {
  AnyTypedArray,
  AnyTypedArrayCtor,
  F32Array,
  F64Array,
  I16Array,
  I32Array,
  I64Array,
  I8Array,
  U16Array,
  U32Array,
  U64Array,
  U8Array,
} from "@xpute/kit/abi/array.ts";
import { ERRNO_EBADMSG, ERRNO_EFAULT } from "@xpute/kit/status/errno.spec.ts";
import { XputeError } from "@xpute/kit/status/error.ts";

import type { AlignUnit, NodeType, NodeValue } from "./spec.ts";
import {
  ALIGN_SZ,
  GET_DESC,
  GET_LEN,
  GET_WORD,
  HDR_SZ,
  LE,
  MAGIC,
  NONE,
  ScalarType,
  SequenceType,
  SpecialType,
  TYPE_IS_LEAF,
  WORD_SZ,
} from "./spec.ts";

type ShallowNodeValue = NodeValue | BranchCursor;

/**
 * Scratchpad for 64-bit word operations to avoid allocation.
 * [lo32, hi32]
 */
const WORD_REG: [u32, u32] = [NONE, NONE];
const DESC_REG: [u32, NodeType] = [NONE, SpecialType.NIL];

// A STR or STRS that is not UTF-8 is refused (EBADMSG), as the Rust and C++
// readers refuse it.
const UTF8 = new TextDecoder("utf-8", { fatal: true });
const utf8 = (bytes: U8Array): string => {
  try {
    return UTF8.decode(bytes);
  } catch {
    throw new XputeError(ERRNO_EBADMSG);
  }
};

const CURSOR = (pkt: U8Array, base: u32, type: NodeType): NodeCursor => {
  const align_sz: AlignUnit = ALIGN_SZ(type);
  if (base % align_sz) throw new XputeError(ERRNO_EBADMSG);
  if (type === SpecialType.BRANCH) return new BranchCursorImpl(pkt, base);
  if (TYPE_IS_LEAF(type)) return new NodeCursorImpl(pkt, base, type);
  throw new XputeError(ERRNO_EBADMSG);
};

const ENTRY_OFF = (base: u32, idx: u32) => base + WORD_SZ + idx * WORD_SZ;

// ============ Reader ============

/**
 * Packet reader and root cursor entry point.
 *
 * Contract:
 * - requires packet base offset to be WORD_SZ-aligned
 * - validates the fixed packet header
 * - returns a root cursor on success
 * - throws on malformed packet
 */
export class TreeReader {
  readonly pkt: U8Array;

  readonly root: NodeCursor;

  constructor(pkt: U8Array) {
    if (pkt.byteOffset % WORD_SZ) throw new XputeError(ERRNO_EBADMSG);

    this.pkt = new Uint8Array(pkt.buffer, pkt.byteOffset, pkt.byteLength);

    if (this.pkt.byteLength < HDR_SZ) throw new XputeError(ERRNO_EBADMSG);
    const hdr_view = new DataView(this.pkt.buffer, this.pkt.byteOffset, HDR_SZ);

    // packet header
    if (GET_LEN(hdr_view, 0) !== MAGIC) throw new XputeError(ERRNO_EBADMSG);

    const [payload_sz, type] = GET_DESC(hdr_view, WORD_SZ, DESC_REG);
    // root header stores payload byte size, not a relative offset
    if (payload_sz > pkt.byteLength - HDR_SZ) {
      throw new XputeError(ERRNO_EBADMSG);
    }

    this.root = payload_sz
      ? CURSOR(this.pkt.subarray(0, HDR_SZ + payload_sz), HDR_SZ, type)
      : new NullCursorImpl(this.pkt, type);
  }

  /** Returns the root cursor after constructor-time packet validation. */
  read(): NodeCursor {
    return this.root;
  }

  read_branch(): BranchCursor {
    return this.root.as_branch();
  }
}

// ============ Cursors ============

export abstract class NodeCursor {
  readonly pkt: U8Array;
  readonly pkt_view: DataView<ArrayBuffer>;
  readonly base: u32;

  readonly type: NodeType;

  /**
   * @param pkt source packet bytes
   * @param base logical node base offset
   */
  constructor(pkt: U8Array, base: u32, type: NodeType) {
    this.pkt = pkt;
    this.pkt_view = new DataView(
      this.pkt.buffer,
      this.pkt.byteOffset,
      this.pkt.byteLength,
    );
    this.base = base;

    this.type = type;
  }

  is_branch(): this is BranchCursor {
    return this.type === SpecialType.BRANCH;
  }

  /**
   * Reinterpret current node as a branch cursor.
   * Fails if the current node is not a branch.
   */
  as_branch(): BranchCursor {
    if (this.type !== SpecialType.BRANCH) throw new XputeError(ERRNO_EBADMSG);
    return this as unknown as BranchCursor;
  }

  // ---- Optional Guard ----

  /**
   * Optional chaining helper.
   * Returns `this` for a present node.
   * NullCursor overrides this to return `undefined`.
   */
  opt(): this | undefined {
    return this;
  }

  // ---- Shallow Getter ----

  /**
   * Reads the current node at shallow depth.
   *
   * Shallow contract:
   * - leaf cursor  -> materialized leaf value
   * - null cursor  -> null
   * - branch cursor is handled by BranchCursor override
   *
   * Use an explicit type parameter when a narrower result type is known.
   */
  get<T extends ShallowNodeValue = ShallowNodeValue>(): T {
    switch (this.type) {
      // -- Scalars --

      case ScalarType.U8:
        return this._u8() as T;
      case ScalarType.I8:
        return this._i8() as T;
      case ScalarType.U16:
        return this._u16() as T;
      case ScalarType.I16:
        return this._i16() as T;
      case ScalarType.U32:
        return this._u32() as T;
      case ScalarType.I32:
        return this._i32() as T;
      case ScalarType.U64:
        return this._u64() as T;
      case ScalarType.I64:
        return this._i64() as T;
      case ScalarType.F32:
        return this._f32() as T;
      case ScalarType.F64:
        return this._f64() as T;
      case ScalarType.BOOL:
        return this._bool() as T;

      // -- Sequences --

      case SequenceType.U8_ARRAY:
        return this._u8_array() as T;
      case SequenceType.I8_ARRAY:
        return this._i8_array() as T;
      case SequenceType.U16_ARRAY:
        return this._u16_array() as T;
      case SequenceType.I16_ARRAY:
        return this._i16_array() as T;
      case SequenceType.U32_ARRAY:
        return this._u32_array() as T;
      case SequenceType.I32_ARRAY:
        return this._i32_array() as T;
      case SequenceType.U64_ARRAY:
        return this._u64_array() as T;
      case SequenceType.I64_ARRAY:
        return this._i64_array() as T;
      case SequenceType.F32_ARRAY:
        return this._f32_array() as T;
      case SequenceType.F64_ARRAY:
        return this._f64_array() as T;
      case SequenceType.BITSET:
        return this._bitset() as T;
      case SequenceType.STR:
        return this._str() as T;
      case SequenceType.STRS:
        return this._strs() as T;

      default:
        throw new XputeError(ERRNO_EBADMSG);
    }
  }

  // ---- Internal Leaf Readers ----

  protected _u8(): u8 {
    if (this.base + 1 > this.pkt.byteLength) {
      throw new XputeError(ERRNO_EBADMSG);
    }
    return this.pkt_view.getUint8(this.base);
  }
  protected _i8(): i8 {
    if (this.base + 1 > this.pkt.byteLength) {
      throw new XputeError(ERRNO_EBADMSG);
    }
    return this.pkt_view.getInt8(this.base);
  }
  protected _u16(): u16 {
    if (this.base + 2 > this.pkt.byteLength) {
      throw new XputeError(ERRNO_EBADMSG);
    }
    return this.pkt_view.getUint16(this.base, LE);
  }
  protected _i16(): i16 {
    if (this.base + 2 > this.pkt.byteLength) {
      throw new XputeError(ERRNO_EBADMSG);
    }
    return this.pkt_view.getInt16(this.base, LE);
  }
  protected _u32(): u32 {
    if (this.base + 4 > this.pkt.byteLength) {
      throw new XputeError(ERRNO_EBADMSG);
    }
    return this.pkt_view.getUint32(this.base, LE);
  }
  protected _i32(): i32 {
    if (this.base + 4 > this.pkt.byteLength) {
      throw new XputeError(ERRNO_EBADMSG);
    }
    return this.pkt_view.getInt32(this.base, LE);
  }
  protected _u64(): u64 {
    if (this.base + 8 > this.pkt.byteLength) {
      throw new XputeError(ERRNO_EBADMSG);
    }
    return this.pkt_view.getBigUint64(this.base, LE);
  }
  protected _i64(): i64 {
    if (this.base + 8 > this.pkt.byteLength) {
      throw new XputeError(ERRNO_EBADMSG);
    }
    return this.pkt_view.getBigInt64(this.base, LE);
  }
  protected _f32(): f32 {
    if (this.base + 4 > this.pkt.byteLength) {
      throw new XputeError(ERRNO_EBADMSG);
    }
    return this.pkt_view.getFloat32(this.base, LE);
  }
  protected _f64(): f64 {
    if (this.base + 8 > this.pkt.byteLength) {
      throw new XputeError(ERRNO_EBADMSG);
    }
    return this.pkt_view.getFloat64(this.base, LE);
  }
  protected _bool(): boolean {
    if (this.base + 1 > this.pkt.byteLength) {
      throw new XputeError(ERRNO_EBADMSG);
    }
    return this.pkt_view.getUint8(this.base) !== 0;
  }

  /**
   * Reads a typed-array sequence leaf as a zero-copy view over the packet buffer.
   *
   * Contract:
   * - checks the len slot's reserved half and len/payload overflow against packet bounds
   * - assumes packet base alignment was validated by TreeReader
   * - sequence payload starts immediately after the len slot: base + WORD_SZ
   * - because sequence node bases are WORD_SZ-aligned, payload_start is also
   *   naturally aligned for all supported typed-array element sizes (<= 8)
   */
  protected _array<T extends AnyTypedArray>(ctor: AnyTypedArrayCtor<T>): T {
    if (this.base + WORD_SZ > this.pkt.byteLength) {
      throw new XputeError(ERRNO_EBADMSG);
    }
    const len = GET_LEN(this.pkt_view, this.base);
    const payload_start: u32 = this.base + WORD_SZ;

    const elem_sz: AlignUnit = ctor.BYTES_PER_ELEMENT as AlignUnit;
    const payload_sz: u32 = len * elem_sz;

    if (payload_start > this.pkt.byteLength) {
      throw new XputeError(ERRNO_EBADMSG);
    }
    if (payload_sz > this.pkt.byteLength - payload_start) {
      throw new XputeError(ERRNO_EBADMSG);
    }

    return new ctor(
      this.pkt_view.buffer,
      this.pkt_view.byteOffset + payload_start,
      len,
    );
  }

  protected _u8_array(): U8Array {
    return this._array(Uint8Array);
  }
  protected _i8_array(): I8Array {
    return this._array(Int8Array);
  }
  protected _u16_array(): U16Array {
    return this._array(Uint16Array);
  }
  protected _i16_array(): I16Array {
    return this._array(Int16Array);
  }
  protected _u32_array(): U32Array {
    return this._array(Uint32Array);
  }
  protected _i32_array(): I32Array {
    return this._array(Int32Array);
  }
  protected _u64_array(): U64Array {
    return this._array(BigUint64Array);
  }
  protected _i64_array(): I64Array {
    return this._array(BigInt64Array);
  }
  protected _f32_array(): F32Array {
    return this._array(Float32Array);
  }
  protected _f64_array(): F64Array {
    return this._array(Float64Array);
  }

  protected _bitset(): U8Array {
    if (this.base + WORD_SZ > this.pkt.byteLength) {
      throw new XputeError(ERRNO_EBADMSG);
    }
    const len = GET_LEN(this.pkt_view, this.base);
    const payload_start: u32 = this.base + WORD_SZ;

    // Computed in doubles, not with `>>> 3`: for a count of 2^32 - 1, `len + 7`
    // would wrap past 2^32 to a size of 0 and pass the bounds check.
    const payload_sz: u32 = Math.floor((len + 7) / 8);
    if (payload_start > this.pkt.byteLength) {
      throw new XputeError(ERRNO_EBADMSG);
    }
    if (payload_sz > this.pkt.byteLength - payload_start) {
      throw new XputeError(ERRNO_EBADMSG);
    }

    const arr: U8Array = new Uint8Array(len);
    for (let i = 0; i < len; i++) {
      const x: u8 = this.pkt[payload_start + (i >>> 3)] & (1 << (i & 7));
      arr[i] = BOOL(x);
    }
    return arr;
  }

  /** Every string is checked as UTF-8, and every offset against the next one
   * and the end, before the strings are returned. */
  protected _strs(): string[] {
    if (this.base + WORD_SZ > this.pkt.byteLength) {
      throw new XputeError(ERRNO_EBADMSG);
    }
    const n = GET_LEN(this.pkt_view, this.base);
    const start: u32 = this.base + WORD_SZ;
    const blob: u32 = start + 4 * (n + 1);
    if (blob > this.pkt.byteLength) throw new XputeError(ERRNO_EBADMSG);
    const off = (i: u32): u32 => this.pkt_view.getUint32(start + 4 * i, true);
    const total = off(n);
    if (total > this.pkt.byteLength - blob) {
      throw new XputeError(ERRNO_EBADMSG);
    }
    const out: string[] = [];
    for (let i = 0; i < n; i++) {
      const [a, b] = [off(i), off(i + 1)];
      if (a > b || b > total) throw new XputeError(ERRNO_EBADMSG);
      out.push(utf8(this.pkt.subarray(blob + a, blob + b)));
    }
    return out;
  }

  // STR stores UTF-8 bytes with a trailing NUL on wire, while len excludes that terminator.
  protected _str(): string {
    if (this.base + WORD_SZ > this.pkt.byteLength) {
      throw new XputeError(ERRNO_EBADMSG);
    }
    const nbyte = GET_LEN(this.pkt_view, this.base);
    const payload_start: u32 = this.base + WORD_SZ;

    if (payload_start > this.pkt.byteLength) {
      throw new XputeError(ERRNO_EBADMSG);
    }
    if (nbyte + 1 > this.pkt.byteLength - payload_start) {
      throw new XputeError(ERRNO_EBADMSG);
    }
    if (this.pkt[payload_start + nbyte] !== 0) {
      throw new XputeError(ERRNO_EBADMSG);
    }

    return utf8(this.pkt.subarray(payload_start, payload_start + nbyte));
  }
}
class NodeCursorImpl extends NodeCursor {}

export abstract class NullCursor extends NodeCursor {
  /**
   * @param pkt source packet bytes
   */
  constructor(pkt: U8Array, type: NodeType) {
    super(pkt, NONE, type);
  }

  // NullCursor preserves original node type metadata,
  // but is never considered a usable structural cursor.
  override is_branch(): this is BranchCursor {
    return false;
  }

  /** An absent node has no branch to read, whatever its type: always EFAULT. */
  override as_branch(): BranchCursor {
    throw new XputeError(ERRNO_EFAULT);
  }

  // ---- Nullable Guard ----

  /**
   * Optional chaining helper.
   * Returns `undefined`: this node is physically absent.
   */
  override opt(): this | undefined {
    return undefined;
  }

  // ---- Shallow Getter ----

  /** Shallow read of a physically absent node reconstructs generic null. */
  override get<T extends ShallowNodeValue = ShallowNodeValue>(): T {
    // generic optional reconstruction
    return null as T;
  }
}
class NullCursorImpl extends NullCursor {}

export abstract class BranchCursor extends NodeCursor {
  override readonly type = SpecialType.BRANCH;

  readonly len: u32;
  readonly child_start: u32; // first byte where child payloads may begin

  constructor(pkt: U8Array, base: u32) {
    super(pkt, base, SpecialType.BRANCH);

    if (base + WORD_SZ > this.pkt.byteLength) {
      throw new XputeError(ERRNO_EBADMSG);
    }
    const len = GET_LEN(this.pkt_view, this.base);

    const table_off = base + WORD_SZ;
    if (len > (pkt.byteLength - table_off) / WORD_SZ) {
      throw new XputeError(ERRNO_EBADMSG);
    }

    this.len = len;
    this.child_start = ENTRY_OFF(this.base, len);
  }

  // ---- Shallow / Deep Getter ----

  /**
   * Shallow branch read.
   *
   * Contract:
   * - get()      -> array of direct children
   * - get(idx)   -> direct child at idx
   * - leaf child -> materialized leaf value
   * - branch child -> BranchCursor from get(); from get(idx), that branch's own shallow array
   * - null child -> null
   */
  override get<T extends ShallowNodeValue[] = ShallowNodeValue[]>(): T;
  override get<T extends ShallowNodeValue = ShallowNodeValue>(idx: u32): T;

  override get<T>(idx?: u32): T {
    if (idx !== undefined) return this.at(idx).get() as unknown as T;

    const arr: (NodeValue | BranchCursor)[] = Array.from(
      { length: this.len },
      (_, i) => {
        const cur: NodeCursor = this.at(i);
        return cur.is_branch() ? cur : cur.get();
      },
    );
    return arr as unknown as T;
  }

  /**
   * Deep branch read.
   *
   * Recursively materializes the entire child subtree into NodeValue[].
   */
  get_deep<T extends NodeValue[] = NodeValue[]>(): T {
    const arr: NodeValue[] = Array.from({ length: this.len }, (_, i) => {
      const cur: NodeCursor = this.at(i);
      return cur.is_branch() ? cur.get_deep() : cur.get();
    });
    return arr as unknown as T;
  }

  /**
   * Returns the child cursor at `idx`.
   *
   * Contract:
   * - preserves child type metadata even when physically absent
   * - returns NullCursor when child_rel_off == 0
   * - validates child offset bounds against the enclosing branch table
   */
  at(idx: u32): NodeCursor {
    if (idx >= this.len) throw new XputeError(ERRNO_EFAULT);
    const entry_off: u32 = ENTRY_OFF(this.base, idx);

    const [rel_off, type] = GET_DESC(this.pkt_view, entry_off, DESC_REG);

    if (rel_off === 0) return new NullCursorImpl(this.pkt, type);
    if (rel_off > this.pkt.byteLength - this.base) {
      throw new XputeError(ERRNO_EBADMSG);
    }
    const base: u32 = this.base + rel_off;
    const end: u32 = this.end_of(idx);
    if (base < this.child_start || base >= end) {
      throw new XputeError(ERRNO_EBADMSG);
    }
    return CURSOR(this.pkt.subarray(0, end), base, type);
  }

  /** Where child `idx`'s extent ends: at the next present sibling's start, or
   * else at this branch's end, where its packet slice ends. */
  private end_of(idx: u32): u32 {
    for (let j = idx + 1; j < this.len; j++) {
      const [rel_off] = GET_WORD(
        this.pkt_view,
        ENTRY_OFF(this.base, j),
        WORD_REG,
      );
      if (rel_off) return Math.min(this.base + rel_off, this.pkt.byteLength);
    }
    return this.pkt.byteLength;
  }

  at_branch(idx: u32): BranchCursor {
    return this.at(idx).as_branch();
  }

  /** Returns all direct child cursors without materializing them. */
  children(): NodeCursor[] {
    return Array.from({ length: this.len }, (_, i) => this.at(i));
  }

  *[Symbol.iterator](): IterableIterator<NodeCursor> {
    for (let i = 0; i < this.len; i++) {
      yield this.at(i);
    }
  }
}
class BranchCursorImpl extends BranchCursor {}
