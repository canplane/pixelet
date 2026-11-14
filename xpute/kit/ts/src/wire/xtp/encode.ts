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
import {
  ERRNO_EBADMSG,
  ERRNO_EINVAL,
  ERRNO_ENOSPC,
  ERRNO_ENOTRECOVERABLE,
  ERRNO_EOVERFLOW,
} from "@xpute/kit/status/errno.spec.ts";
import { XputeError } from "@xpute/kit/status/error.ts";
import { bug } from "@xpute/kit/status/bug.ts";

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

// ============ Encoder ============

export interface TreeEncoderConfig {
  init_cap?: u32; // soft initial buffer size hint
  max_cap?: u32; // hard packet size limit
}

// The encoder assumes the normal construction path went through the view layer.
// It still enforces minimal packet-boundary invariants: the cap, the node type, and a graft's header and payload size.
export class TreeEncoder {
  // ---- Entry ----

  /** A `max_cap` below HDR_SZ or above MAX_PKT_SZ is EINVAL; a packet larger
   * than `max_cap` is EOVERFLOW. */
  encode(view: NodeView, cfg: TreeEncoderConfig = {}): U8Array {
    const { init_cap = 1 << 10, max_cap = MAX_PKT_SZ } = cfg;
    if (max_cap < HDR_SZ || max_cap > MAX_PKT_SZ) {
      throw new XputeError(ERRNO_EINVAL);
    }
    let cap: u32 = Math.max(HDR_SZ, Math.min(init_cap, max_cap));
    for (;;) {
      const buf: U8Array = new Uint8Array(cap);
      const w = new PacketWriter().root(buf, new DataView(buf.buffer), 0, cap);
      walk(w, view.node);
      try {
        return buf.subarray(0, w.finish_or_throw());
      } catch (e) {
        if (
          !(e instanceof XputeError) ||
          e.errnum !== ERRNO_EOVERFLOW ||
          cap === max_cap
        )
          throw e;
        cap = cap > max_cap / 2 ? max_cap : cap * 2;
      }
    }
  }
}

export const encoder = new TreeEncoder();

// ---- Node Dispatch ----

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
    throw new XputeError(ERRNO_EBADMSG);
  }
}

// ---- Leaf Dispatch ----

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
      throw new XputeError(ERRNO_EBADMSG);
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
      throw new XputeError(ERRNO_EBADMSG);
  }
}

/** The child table the writer's next entries go in. The root is treated as a
 * branch of one child whose entry is the header's root word: `root` marks it,
 * and its entry sets the root's type instead of writing a descriptor. */
interface Table {
  base: u32;
  cnt: u32;
  written: u32;
  root: boolean;
}

const word_up = (n: number): number => Math.ceil(n / WORD_SZ) * WORD_SZ;

/**
 * Writes a packet in place, into bytes the caller holds, without building a
 * tree first. It is the one writer of the layout: `TreeEncoder` walks its tree
 * onto it, so both give the same bytes. A node that does not fit does not
 * fail its call: the writer goes on counting entries, and `finish_or_throw`
 * refuses the packet (EOVERFLOW), `finish` answering -1. One writer object is reused: its room begins it again
 * for each packet.
 */
export class PacketWriter {
  private bytes: U8Array = new Uint8Array(0);
  private view: DataView = new DataView(new ArrayBuffer(0));
  /** This is where the packet begins in `bytes`. */
  start: u32 = 0;
  /** This is the most bytes the packet may take. */
  private cap: u32 = 0;
  private table: Table = { base: 0, cnt: 1, written: 0, root: true };
  private root_type: NodeType = SpecialType.NIL;
  /** The packet's end so far, counted from `start`. */
  private lim: u32 = HDR_SZ;
  /** This is the first error met; later ones are not kept. */
  private err: i32 | null = null;

  /** Begins a packet whose root is any one node, at `start` in `bytes`.
   * `bytes` and `view` must see the same memory from the same offset. The
   * packet may take `cap` bytes, up to MAX_PKT_SZ. */
  root(bytes: U8Array, view: DataView, start: u32, cap: u32): this {
    this.bytes = bytes;
    this.view = view;
    this.start = start;
    this.cap = Math.min(cap, MAX_PKT_SZ);
    this.table = { base: 0, cnt: 1, written: 0, root: true };
    this.root_type = SpecialType.NIL;
    this.lim = HDR_SZ;
    this.err = null;
    if (this.cap < HDR_SZ) this.spoil(ERRNO_EINVAL);
    else bytes.fill(0, start, start + HDR_SZ);
    return this;
  }

  /** Begins a packet whose root is a branch of `cnt` children, which the
   * calls that follow write. */
  begin(bytes: U8Array, view: DataView, start: u32, cap: u32, cnt: u32): this {
    this.root(bytes, view, start, cap);
    const at = this.branch_open(cnt);
    this.root_type = SpecialType.BRANCH;
    this.table = { base: at < 0 ? HDR_SZ : at, cnt, written: 0, root: false };
    return this;
  }

  private spoil(errnum: i32): -1 {
    this.err ??= errnum;
    return -1;
  }

  /** Takes `bytes` at the next multiple of `unit`, zeroing them and the
   * padding before them. It returns where they begin, or -1 when they do not
   * fit. */
  private reserve(unit: AlignUnit, bytes: number): number {
    if (this.err !== null) return -1;
    const base: u32 = ALIGN(this.lim, unit);
    const end: number = base + bytes;
    if (end > this.cap) return this.spoil(ERRNO_EOVERFLOW);
    this.bytes.fill(0, this.start + this.lim, this.start + end);
    this.lim = end;
    return base;
  }

  private entry(at: number, type: NodeType): this {
    const f = this.table;
    if (f.written >= f.cnt) bug(ERRNO_ENOSPC);
    if (f.root) this.root_type = type;
    else if (this.err === null) {
      // child node kind is preserved even when rel_off == 0
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

  // scalar layout:
  // [payload]
  // - scalar nodes have no header
  // - scalar alignment is equal to the element size
  private scalar(
    type: ScalarType,
    unit: AlignUnit,
    set: ((at: u32) => void) | null,
  ): this {
    const at = set === null ? -1 : this.reserve(unit, unit);
    if (at >= 0) set!(this.start + at);
    return this.entry(at, type);
  }

  // sequence layout:
  // [len 32 | reserved 32][payload...]
  // - len is stored in an 8-byte slot
  // - the value is the low 32 bits; a reader refuses a len slot whose high half is not zero
  // - payload begins immediately after the len slot: base + WORD_SZ
  // - sequence node base is WORD_SZ-aligned
  // - STR stores a trailing NUL on wire, but len excludes that terminator
  // - `fill` gets the payload's offset in `bytes` and writes it
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

  /** `a`'s bytes are copied as they lie, so the host must be little-endian,
   * as the wire is. */
  array(type: SequenceType, a: TypedArray | BigTypedArray | null): this {
    if (a === null) return this.entry(-1, type);
    return this.seq(type, a.length, a.byteLength, (payload) =>
      this.bytes.set(
        new Uint8Array(a.buffer, a.byteOffset, a.byteLength),
        payload,
      ),
    );
  }

  /** `bits` holds one byte for each bit, nonzero for set. They are packed
   * eight to a byte, the first in the lowest bit. */
  bitset(bits: U8Array | null): this {
    if (bits === null) return this.entry(-1, SequenceType.BITSET);
    return this.seq(
      SequenceType.BITSET,
      bits.length,
      Math.ceil(bits.length / 8),
      (payload) => {
        // bit packing: bit i -> byte[i >> 3], bit position (i & 7), LSB-first within each byte
        bits.forEach((b, i) => {
          if (b) this.bytes[payload + (i >>> 3)] |= 1 << (i & 7);
        });
      },
    );
  }

  /** The string's UTF-8 as `TextEncoder` writes it, in which a lone surrogate
   * becomes U+FFFD. */
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
    // wire payload includes trailing NUL for C-friendly reads
    const end: number = res === null ? 0 : payload + word_up(res.written + 1);
    if (res === null || res.read < s.length || end > this.cap) {
      return this.entry(this.spoil(ERRNO_EOVERFLOW), SequenceType.STR);
    }
    this.bytes.fill(0, this.start + this.lim, this.start + payload);
    this.bytes.fill(0, this.start + payload + res.written, this.start + end);
    SET_WORD(this.view, this.start + base, res.written, RESERVED); // UTF-8 byte length, excluding trailing NUL
    this.lim = end;
    return this.entry(base, SequenceType.STR);
  }

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

  /** A branch's extent ends on a word boundary, so a node after it starts on
   * the next word even when it needs less alignment. Every language pads the
   * same way, which keeps their bytes equal. */
  private branch_close(at: number): number {
    if (at < 0 || this.err !== null) return at;
    const end: u32 = ALIGN(this.lim, WORD_SZ);
    if (end > this.cap) return this.spoil(ERRNO_EOVERFLOW);
    this.bytes.fill(0, this.start + this.lim, this.start + end);
    this.lim = end;
    return at;
  }

  // ---- Branch Encoding ----

  // branch layout:
  // [len_lo32 | reserved_hi32]
  // [rel_off_lo32 | desc_hi32]...
  //
  // desc_hi32:
  //   bits 0..7   : node type
  //   bits 8..31  : reserved
  //
  // - len and offsets occupy 8-byte slots
  // - the value is the low 32 bits; a reader refuses a len slot whose high half is not zero,
  //   and an offset's high half holds the type
  // - each child offset is relative to the enclosing branch node base
  // - branch alignment is WORD_SZ
  // - writing a number of children other than `cnt` is a bug
  branch(cnt: u32, fill: (w: this) => void): this {
    // empty branch (len = 0) is valid — analogous to [] or {} in JSON
    const at = this.branch_open(cnt);
    // When the branch did not fit, its children are still counted, so that a
    // miscount is caught; the packet is spoiled, and nothing is written at the
    // stand-in base.
    const outer = this.table;
    this.table = { base: at < 0 ? HDR_SZ : at, cnt, written: 0, root: false };
    fill(this);
    if (this.table.written !== this.table.cnt) {
      bug(ERRNO_ENOTRECOVERABLE);
    }
    this.table = outer;
    return this.entry(this.branch_close(at), SpecialType.BRANCH);
  }

  no_branch(): this {
    return this.entry(-1, SpecialType.BRANCH);
  }

  // graft semantics:
  // - `pkt` is a complete tree packet
  // - the outer packet header is stripped
  // - only the grafted root payload region is copied into the current packet
  // - packet header word1 stores [root_payload_sz 32 | type 8 | reserved 24]
  // - placement alignment follows the grafted root node type
  // - because root_payload_sz excludes the fixed packet header, graft preserves
  //   the same root payload layout as inline encoding
  // - base-offset alignment is expected to be enforced by the view construction path
  // - a malformed `pkt` makes `finish_or_throw` refuse the packet (EBADMSG), and `finish` answer -1
  graft(pkt: U8Array): this {
    let payload_sz: u32, type: NodeType;
    try {
      if (pkt.byteLength < HDR_SZ) throw new XputeError(ERRNO_EBADMSG);
      const hdr = new DataView(pkt.buffer, pkt.byteOffset, HDR_SZ);
      // packet header fallback
      if (GET_LEN(hdr, 0) !== MAGIC) throw new XputeError(ERRNO_EBADMSG);
      [payload_sz, type] = GET_DESC(hdr, WORD_SZ, DESC_REG);
    } catch (e) {
      if (!(e instanceof XputeError)) throw e;
      return this.entry(this.spoil(ERRNO_EBADMSG), SpecialType.NIL);
    }
    if (payload_sz > pkt.byteLength - HDR_SZ) {
      return this.entry(this.spoil(ERRNO_EBADMSG), type);
    }
    if (payload_sz === 0) return this.entry(-1, type);
    let unit: AlignUnit;
    try {
      unit = ALIGN_SZ(type);
    } catch (e) {
      if (!(e instanceof XputeError)) throw e;
      return this.entry(this.spoil(ERRNO_EBADMSG), type);
    }
    if (payload_sz % unit) return this.entry(this.spoil(ERRNO_EBADMSG), type);
    const at = this.reserve(unit, payload_sz);
    if (at >= 0) {
      this.bytes.set(
        pkt.subarray(HDR_SZ, HDR_SZ + payload_sz),
        this.start + at,
      );
    }
    return this.entry(at, type);
  }

  /** Writes the header and returns the packet's length, header included and
   * padded to a word, or throws the first error that spoiled the packet. */
  finish_or_throw(): u32 {
    const f = this.table;
    if (f.written !== f.cnt) bug(ERRNO_ENOTRECOVERABLE);
    // The root branch that `begin` opened is closed here, padded as `branch`
    // pads one.
    if (!f.root) this.branch_close(f.base);
    if (this.err !== null) throw new XputeError(this.err);
    const end: u32 = ALIGN(this.lim, WORD_SZ);
    if (end > this.cap) throw new XputeError(ERRNO_EOVERFLOW);
    this.bytes.fill(0, this.start + this.lim, this.start + end);
    SET_WORD(this.view, this.start, MAGIC, RESERVED); // word0
    // root node kind is preserved even when payload_sz == 0
    SET_WORD(
      this.view,
      this.start + WORD_SZ,
      this.lim - HDR_SZ,
      FIELD_SET(0, DESC_TYPE_SHAMT, DESC_TYPE_MASK, this.root_type),
    ); // word1
    return end;
  }

  /** This answers `finish_or_throw`'s length, or -1 when the packet was
   * refused, for a caller that only needs to know whether the packet fit. */
  finish(): number {
    try {
      return this.finish_or_throw();
    } catch (e) {
      if (e instanceof XputeError) return -1;
      throw e;
    }
  }
}
