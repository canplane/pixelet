// @xpute/kit/wire/xtp/encode.ts

import {
  BOOL,
  type f32,
  type f64,
  FIELD_SET,
  type i16,
  type i32,
  type i64,
  type i8,
  type u16,
  type u32,
  type u64,
  type u8,
} from "@xpute/kit/abi/word.ts";
import type {
  BigTypedArray,
  TypedArray,
  U8Array,
} from "@xpute/kit/abi/array.ts";
import * as encoding from "@xpute/kit/codec/encoding.ts";
import { Errno } from "@xpute/kit/status/errno.spec.ts";
import { InvariantError, MarshalError } from "@xpute/kit/status/error.ts";

import type {
  AlignUnit,
  Node,
  NodeType,
  ScalarNode,
  SequenceNode,
} from "./spec.ts";
import {
  ALIGN,
  ALIGN_SZ,
  DESC_TYPE_MASK,
  DESC_TYPE_SHAMT,
  GET_DESC,
  GET_LEN,
  HDR_SZ,
  LE,
  MAGIC,
  MAX_PKT_SZ,
  NODE_IS_BRANCH,
  NODE_IS_GRAFT,
  NODE_IS_NIL,
  NODE_IS_SCALAR,
  NODE_IS_SEQ,
  NONE,
  RESERVED,
  ScalarType,
  SequenceType,
  SET_WORD,
  SpecialType,
  WORD_SZ,
} from "./spec.ts";
import type { NodeView } from "./view.ts";

const DESC_REG: [u32, NodeType] = [NONE, SpecialType.NIL];

export interface TreeEncoderOptions {
  /** The first buffer's size; it doubles where the tree overflows it. */
  init_cap: u32;
  max_cap?: u32; // hard packet size limit
}

/**
 * A tree walked onto `PacketWriter`, the one writer of the layout, as the C++
 * encoder does (cpp kit wire/xtp/view.hpp `encode`): into a fixed buffer,
 * walked again on one twice as large where it overflowed, up to the cap.
 */
export class TreeEncoder {
  /** EINVAL for a cap outside a packet's sizes, EOVERFLOW for a tree past the cap. */
  encode(view: NodeView, opts: TreeEncoderOptions): U8Array {
    const { init_cap, max_cap = MAX_PKT_SZ } = opts;
    if (max_cap < HDR_SZ || max_cap > MAX_PKT_SZ) {
      throw new MarshalError(Errno.EINVAL);
    }
    let cap: u32 = Math.max(HDR_SZ, Math.min(init_cap, max_cap));
    for (;;) {
      const buf: U8Array = new Uint8Array(cap);
      const w = new PacketWriter().root(buf, new DataView(buf.buffer), 0, cap);
      walk(w, view.node);
      try {
        return buf.subarray(0, w.finished());
      } catch (e) {
        if (
          !(e instanceof MarshalError) ||
          e.errnum !== Errno.EOVERFLOW ||
          cap === max_cap
        )
          throw e;
        cap = cap > max_cap / 2 ? max_cap : cap * 2;
      }
    }
  }
}

export const encoder = new TreeEncoder();

/** A node written as the entry the writer is at. */
function walk(w: PacketWriter, node: Node): void {
  if (NODE_IS_NIL(node)) {
    w.nil();
  } else if (NODE_IS_BRANCH(node)) {
    const children = node.val;
    if (children === null) w.no_branch();
    else w.branch(children.length, (b) => children.forEach((c) => walk(b, c)));
  } else if (NODE_IS_GRAFT(node)) {
    w.graft(node.val);
  } else if (NODE_IS_SEQ(node)) {
    seq_into(w, node);
  } else if (NODE_IS_SCALAR(node)) {
    scalar_into(w, node);
  } else {
    throw new MarshalError(Errno.EBADMSG);
  }
}

function scalar_into(w: PacketWriter, node: ScalarNode): void {
  const v = node.val;
  switch (node.type) {
    case ScalarType.U8:
      w.u8(v as u8 | null);
      return;
    case ScalarType.I8:
      w.i8(v as i8 | null);
      return;
    case ScalarType.U16:
      w.u16(v as u16 | null);
      return;
    case ScalarType.I16:
      w.i16(v as i16 | null);
      return;
    case ScalarType.U32:
      w.u32(v as u32 | null);
      return;
    case ScalarType.I32:
      w.i32(v as i32 | null);
      return;
    case ScalarType.U64:
      w.u64(v as u64 | null);
      return;
    case ScalarType.I64:
      w.i64(v as i64 | null);
      return;
    case ScalarType.F32:
      w.f32(v as f32 | null);
      return;
    case ScalarType.F64:
      w.f64(v as f64 | null);
      return;
    case ScalarType.BOOL:
      w.bool(v === null ? null : BOOL(v) === 1);
      return;
    default:
      throw new MarshalError(Errno.EBADMSG);
  }
}

function seq_into(w: PacketWriter, node: SequenceNode): void {
  const v = node.val;
  switch (node.type) {
    case SequenceType.STR:
      w.str(v as string | null);
      return;
    case SequenceType.STRS:
      w.strs(v as string[] | null);
      return;
    case SequenceType.BITSET:
      w.bitset(v as U8Array | null);
      return;
    case SequenceType.U8_ARRAY:
    case SequenceType.I8_ARRAY:
    case SequenceType.U16_ARRAY:
    case SequenceType.I16_ARRAY:
    case SequenceType.U32_ARRAY:
    case SequenceType.I32_ARRAY:
    case SequenceType.U64_ARRAY:
    case SequenceType.I64_ARRAY:
    case SequenceType.F32_ARRAY:
    case SequenceType.F64_ARRAY:
      w.array(node.type, v as TypedArray | BigTypedArray | null);
      return;
    default:
      throw new MarshalError(Errno.EBADMSG);
  }
}

/** Where a packet is written in place: a writer begun over the room for a
 * root branch of `cnt` children. */
export interface PacketRoom {
  writer(cnt: u32): PacketWriter;
}

/** The table a writer's entries go in: a branch's, or the root's, a branch of
 * one whose entry is the header. */
interface Frame {
  base: u32;
  cnt: u32;
  written: u32;
  root: boolean;
}

const word_up = (n: number): number => Math.ceil(n / WORD_SZ) * WORD_SZ;

/**
 * Writes a packet in place, into bytes the caller holds, with no tree built
 * first: the one writer of the layout, which `TreeEncoder` walks its tree onto
 * (rust/kit/src/wire/xtp/encode.rs `PacketWriter`, cpp kit wire/xtp/encode.hpp).
 * A branch declares its child count up front; a child that does not fit makes
 * `finish` refuse, and a miscount throws. One writer serves every packet its
 * room hands out, begun again for each.
 */
export class PacketWriter {
  private bytes: U8Array = new Uint8Array(0);
  private view: DataView = new DataView(new ArrayBuffer(0));
  /** Where the packet begins in `bytes`, and the bytes it may take. */
  start: u32 = 0;
  private cap: u32 = 0;
  private frame: Frame = { base: 0, cnt: 1, written: 0, root: true };
  private root_type: NodeType = SpecialType.NIL;
  /** The packet's end so far, from `start`. */
  private lim: u32 = HDR_SZ;
  /** The first reason only. */
  private err: Errno | null = null;

  /** A packet of one root, any node. `bytes` and `view` see the same memory
   * from the same offset; `cap` is at most MAX_PKT_SZ. */
  root(bytes: U8Array, view: DataView, start: u32, cap: u32): this {
    this.bytes = bytes;
    this.view = view;
    this.start = start;
    this.cap = Math.min(cap, MAX_PKT_SZ);
    this.frame = { base: 0, cnt: 1, written: 0, root: true };
    this.root_type = SpecialType.NIL;
    this.lim = HDR_SZ;
    this.err = null;
    if (this.cap < HDR_SZ) this.spoil(Errno.EINVAL);
    else bytes.fill(0, start, start + HDR_SZ);
    return this;
  }

  /** A packet whose root is a branch of `cnt`, its children written next. */
  begin(
    bytes: U8Array,
    view: DataView,
    start: u32,
    cap: u32,
    cnt: u32,
  ): this {
    this.root(bytes, view, start, cap);
    const at = this.branch_open(cnt);
    this.root_type = SpecialType.BRANCH;
    this.frame = { base: at < 0 ? HDR_SZ : at, cnt, written: 0, root: false };
    return this;
  }

  private spoil(errnum: Errno): -1 {
    this.err ??= errnum;
    return -1;
  }

  /** Where `bytes` aligned to `unit` begin, zeroed from the end so far; -1 past the room. */
  private reserve(unit: AlignUnit, bytes: number): number {
    if (this.err !== null) return -1;
    const base: u32 = ALIGN(this.lim, unit);
    const end: number = base + bytes;
    if (end > this.cap) return this.spoil(Errno.EOVERFLOW);
    this.bytes.fill(0, this.start + this.lim, this.start + end);
    this.lim = end;
    return base;
  }

  private entry(at: number, type: NodeType): this {
    const f = this.frame;
    if (f.written >= f.cnt) throw new InvariantError(Errno.ENOSPC);
    if (f.root) this.root_type = type;
    else if (this.err === null) {
      SET_WORD(
        this.view,
        this.start + f.base + WORD_SZ * (1 + f.written),
        at < 0 ? 0 : at - f.base,
        FIELD_SET(0, DESC_TYPE_SHAMT, DESC_TYPE_MASK, type),
      );
    }
    f.written++;
    return this;
  }

  private scalar(
    type: ScalarType,
    unit: AlignUnit,
    set: ((at: u32) => void) | null,
  ): this {
    const at = set === null ? -1 : this.reserve(unit, unit);
    if (at >= 0) set!(this.start + at);
    return this.entry(at, type);
  }

  /** A sequence's length word and `bytes` of payload after it, filled by `fill` at the payload's place in `bytes`. */
  private seq(
    type: SequenceType,
    len: u32,
    bytes: number,
    fill: (payload: number) => void,
  ): this {
    const at = this.reserve(WORD_SZ, WORD_SZ + word_up(bytes));
    if (at >= 0) {
      SET_WORD(this.view, this.start + at, len, RESERVED);
      fill(this.start + at + WORD_SZ);
    }
    return this.entry(at, type);
  }

  nil(): this {
    return this.entry(-1, SpecialType.NIL);
  }

  u8(v: u8 | null): this {
    return this.scalar(
      ScalarType.U8,
      1,
      v === null ? null : (at) => this.view.setUint8(at, v),
    );
  }

  i8(v: i8 | null): this {
    return this.scalar(
      ScalarType.I8,
      1,
      v === null ? null : (at) => this.view.setInt8(at, v),
    );
  }

  u16(v: u16 | null): this {
    return this.scalar(
      ScalarType.U16,
      2,
      v === null ? null : (at) => this.view.setUint16(at, v, LE),
    );
  }

  i16(v: i16 | null): this {
    return this.scalar(
      ScalarType.I16,
      2,
      v === null ? null : (at) => this.view.setInt16(at, v, LE),
    );
  }

  u32(v: u32 | null): this {
    return this.scalar(
      ScalarType.U32,
      4,
      v === null ? null : (at) => this.view.setUint32(at, v, LE),
    );
  }

  i32(v: i32 | null): this {
    return this.scalar(
      ScalarType.I32,
      4,
      v === null ? null : (at) => this.view.setInt32(at, v, LE),
    );
  }

  u64(v: u64 | null): this {
    return this.scalar(
      ScalarType.U64,
      8,
      v === null ? null : (at) => this.view.setBigUint64(at, v, LE),
    );
  }

  i64(v: i64 | null): this {
    return this.scalar(
      ScalarType.I64,
      8,
      v === null ? null : (at) => this.view.setBigInt64(at, v, LE),
    );
  }

  f32(v: f32 | null): this {
    return this.scalar(
      ScalarType.F32,
      4,
      v === null ? null : (at) => this.view.setFloat32(at, v, LE),
    );
  }

  f64(v: f64 | null): this {
    return this.scalar(
      ScalarType.F64,
      8,
      v === null ? null : (at) => this.view.setFloat64(at, v, LE),
    );
  }

  bool(v: boolean | null): this {
    return this.scalar(
      ScalarType.BOOL,
      1,
      v === null ? null : (at) => this.view.setUint8(at, BOOL(v)),
    );
  }

  /** An array of `type`, its elements as they lie in `a`, little-endian. */
  array(type: SequenceType, a: TypedArray | BigTypedArray | null): this {
    if (a === null) return this.entry(-1, type);
    return this.seq(type, a.length, a.byteLength, (payload) =>
      this.bytes.set(
        new Uint8Array(a.buffer, a.byteOffset, a.byteLength),
        payload,
      ),
    );
  }

  /** One byte a bit, packed low bit first. */
  bitset(bits: U8Array | null): this {
    if (bits === null) return this.entry(-1, SequenceType.BITSET);
    return this.seq(
      SequenceType.BITSET,
      bits.length,
      Math.ceil(bits.length / 8),
      (payload) => {
        bits.forEach((b, i) => {
          if (b) this.bytes[payload + (i >>> 3)] |= 1 << (i & 7);
        });
      },
    );
  }

  /** UTF-8 as `TextEncoder` writes it, a NUL after it that its length leaves out. */
  str(s: string | null): this {
    if (s === null || this.err !== null) {
      return this.entry(-1, SequenceType.STR);
    }
    const base: u32 = ALIGN(this.lim, WORD_SZ);
    const payload: u32 = base + WORD_SZ;
    const res =
      payload < this.cap
        ? encoding.te.encodeInto(
            s,
            this.bytes.subarray(this.start + payload, this.start + this.cap),
          )
        : null;
    const end: number = res === null ? 0 : payload + word_up(res.written + 1);
    if (res === null || res.read < s.length || end > this.cap) {
      return this.entry(this.spoil(Errno.EOVERFLOW), SequenceType.STR);
    }
    this.bytes.fill(0, this.start + this.lim, this.start + payload);
    this.bytes.fill(0, this.start + payload + res.written, this.start + end);
    SET_WORD(this.view, this.start + base, res.written, RESERVED);
    this.lim = end;
    return this.entry(base, SequenceType.STR);
  }

  /** `len` + 1 offsets into the UTF-8 after them. */
  strs(items: string[] | null): this {
    if (items === null) return this.entry(-1, SequenceType.STRS);
    const utf8 = items.map((s) => encoding.te.encode(s));
    const lane: u32 = 4 * (utf8.length + 1);
    return this.seq(
      SequenceType.STRS,
      utf8.length,
      lane + utf8.reduce((n, b) => n + b.byteLength, 0),
      (payload) => {
        let off: u32 = 0;
        this.view.setUint32(payload, 0, LE);
        utf8.forEach((b, i) => {
          this.bytes.set(b, payload + lane + off);
          off += b.byteLength;
          this.view.setUint32(payload + 4 * (i + 1), off, LE);
        });
      },
    );
  }

  private branch_open(cnt: u32): number {
    const at = this.reserve(WORD_SZ, WORD_SZ * (1 + cnt));
    if (at >= 0) SET_WORD(this.view, this.start + at, cnt, RESERVED);
    return at;
  }

  /** A branch ends on a word, so what follows it starts where the encoder starts it. */
  private branch_close(at: number): number {
    if (at < 0 || this.err !== null) return at;
    const end: u32 = ALIGN(this.lim, WORD_SZ);
    if (end > this.cap) return this.spoil(Errno.EOVERFLOW);
    this.bytes.fill(0, this.start + this.lim, this.start + end);
    this.lim = end;
    return at;
  }

  /** Writing a different number of children than `cnt` throws. */
  branch(cnt: u32, fill: (w: this) => void): this {
    const at = this.branch_open(cnt);
    // A branch that did not fit still counts its children, against a base nothing is written at.
    const outer = this.frame;
    this.frame = { base: at < 0 ? HDR_SZ : at, cnt, written: 0, root: false };
    fill(this);
    if (this.frame.written !== this.frame.cnt) {
      throw new InvariantError(Errno.ENOTRECOVERABLE);
    }
    this.frame = outer;
    return this.entry(this.branch_close(at), SpecialType.BRANCH);
  }

  no_branch(): this {
    return this.entry(-1, SpecialType.BRANCH);
  }

  /** Its root's payload copied, placed as its root's type places it. */
  graft(pkt: U8Array): this {
    let payload_sz: u32, type: NodeType;
    try {
      if (pkt.byteLength < HDR_SZ) throw new MarshalError(Errno.EBADMSG);
      const hdr = new DataView(pkt.buffer, pkt.byteOffset, HDR_SZ);
      if (GET_LEN(hdr, 0) !== MAGIC) throw new MarshalError(Errno.EBADMSG);
      [payload_sz, type] = GET_DESC(hdr, WORD_SZ, DESC_REG);
    } catch (e) {
      if (!(e instanceof MarshalError)) throw e;
      return this.entry(this.spoil(Errno.EBADMSG), SpecialType.NIL);
    }
    if (payload_sz > pkt.byteLength - HDR_SZ) {
      return this.entry(this.spoil(Errno.EBADMSG), type);
    }
    if (payload_sz === 0) return this.entry(-1, type);
    let unit: AlignUnit;
    try {
      unit = ALIGN_SZ(type);
    } catch (e) {
      if (!(e instanceof MarshalError)) throw e;
      return this.entry(this.spoil(Errno.EBADMSG), type);
    }
    if (payload_sz % unit) return this.entry(this.spoil(Errno.EBADMSG), type);
    const at = this.reserve(unit, payload_sz);
    if (at >= 0) {
      this.bytes.set(
        pkt.subarray(HDR_SZ, HDR_SZ + payload_sz),
        this.start + at,
      );
    }
    return this.entry(at, type);
  }

  /** The length, header included and to the word; throws why it was refused. */
  finished(): u32 {
    const f = this.frame;
    if (f.written !== f.cnt) throw new InvariantError(Errno.ENOTRECOVERABLE);
    // The root branch `begin` opened, closed as `branch` closes one.
    if (!f.root) this.branch_close(f.base);
    if (this.err !== null) throw new MarshalError(this.err);
    const end: u32 = ALIGN(this.lim, WORD_SZ);
    if (end > this.cap) throw new MarshalError(Errno.EOVERFLOW);
    this.bytes.fill(0, this.start + this.lim, this.start + end);
    SET_WORD(this.view, this.start, MAGIC, RESERVED);
    SET_WORD(
      this.view,
      this.start + WORD_SZ,
      this.lim - HDR_SZ,
      FIELD_SET(0, DESC_TYPE_SHAMT, DESC_TYPE_MASK, this.root_type),
    );
    return end;
  }

  /** The packet's length, or -1 when a child did not fit: for a room that fits a packet or not. */
  finish(): number {
    try {
      return this.finished();
    } catch (e) {
      if (e instanceof MarshalError) return -1;
      throw e;
    }
  }
}
