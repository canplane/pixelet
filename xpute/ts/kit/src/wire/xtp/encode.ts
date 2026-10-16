// @xpute/kit/wire/xtp/encode.ts

import { BOOL, type f32, type f64, FIELD_SET, type i16, type i32, type i64, type i8, type u16, type u32, type u64, type u8 } from "@xpute/kit/abi/word.ts";
import type { BigTypedArray, TypedArray, U8Array } from "@xpute/kit/abi/array.ts";
import * as encoding from "@xpute/kit/codec/encoding.ts";
import { Errno } from "@xpute/kit/status/errno.spec.ts";
import { InvariantError, MarshalError } from "@xpute/kit/status/error.ts";

import type { AlignUnit, BranchNode, EncodingState, GraftNode, Node, NodeType, ScalarNode, SequenceNode } from "./spec.ts";
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
  NODE_IS_LEAF,
  NODE_IS_NIL,
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
  init_cap?: u32;
  max_cap?: u32; // hard packet size limit
}

export class TreeEncoder {
  max_cap!: u32;

  buf!: U8Array;
  buf_view!: DataView<ArrayBuffer>;

  private _ensure(new_cap: u32): u32 {
    const cap: u32 = this.buf.byteLength;
    if (new_cap <= cap) return cap;

    // Doubling stops at the cap, so what the cap refuses is a packet that
    // does not fit and never a growth step past it.
    if (new_cap > this.max_cap) throw new MarshalError(Errno.EOVERFLOW);
    new_cap = Math.min(Math.max(cap * 2, new_cap), this.max_cap);

    const new_buf: U8Array = new Uint8Array(new_cap);
    new_buf.set(this.buf);

    this.buf = new_buf;
    this.buf_view = new DataView(this.buf.buffer, this.buf.byteOffset, this.buf.byteLength);
    return this.buf.byteLength;
  }

  encode(view: NodeView, opts: TreeEncoderOptions = {}): U8Array {
    const { node } = view;

    const { init_cap: _init_cap = 1 << 10, max_cap = MAX_PKT_SZ } = opts;
    if (max_cap < HDR_SZ || max_cap > MAX_PKT_SZ) throw new MarshalError(Errno.EINVAL);
    this.max_cap = max_cap;

    const init_cap: u32 = Math.max(HDR_SZ, Math.min(_init_cap, this.max_cap));
    this.buf = new Uint8Array(init_cap);
    this.buf_view = new DataView(this.buf.buffer, this.buf.byteOffset, this.buf.byteLength);

    SET_WORD(this.buf_view, 0, MAGIC, RESERVED);

    const st: EncodingState = { base: HDR_SZ, lim: HDR_SZ, type: node.type };
    this._node(node, st);

    // A grafted root takes the type its packet's header carries.
    const payload_sz: u32 = st.lim - HDR_SZ;
    const desc: u32 = FIELD_SET(0, DESC_TYPE_SHAMT, DESC_TYPE_MASK, st.type);
    SET_WORD(this.buf_view, WORD_SZ, payload_sz, desc);

    const pkt_nbyte: u32 = ALIGN(HDR_SZ + payload_sz, WORD_SZ);
    this._ensure(pkt_nbyte);
    return this.buf.subarray(0, pkt_nbyte);
  }

  private _node(node: Node, st: EncodingState): void {
    if (NODE_IS_LEAF(node)) {
      if (NODE_IS_SEQ(node)) return this._seq(node, st);
      return this._scalar(node, st);
    }

    if (NODE_IS_BRANCH(node)) return this._branch(node, st);
    if (NODE_IS_GRAFT(node)) return this._graft(node, st);
    if (NODE_IS_NIL(node)) return;
    throw new MarshalError(Errno.EBADMSG);
  }

  // Only the grafted packet's root payload is copied, so it lays out as the inline subtree would.
  private _graft(node: GraftNode, st: EncodingState): void {
    const { val: pkt } = node;

    if (pkt.byteLength < HDR_SZ) throw new MarshalError(Errno.EBADMSG);
    const hdr_view = new DataView(pkt.buffer, pkt.byteOffset, HDR_SZ);

    if (GET_LEN(hdr_view, 0) !== MAGIC) throw new MarshalError(Errno.EBADMSG);

    const [payload_sz, type] = GET_DESC(hdr_view, WORD_SZ, DESC_REG);
    if (payload_sz > pkt.byteLength - HDR_SZ) throw new MarshalError(Errno.EBADMSG);
    st.type = type;

    if (payload_sz === 0) return;

    const align_sz: AlignUnit = ALIGN_SZ(st.type);
    if (payload_sz % align_sz) throw new MarshalError(Errno.EBADMSG);

    st.base = ALIGN(st.base, align_sz);

    if (payload_sz > this.max_cap - st.base) throw new MarshalError(Errno.EOVERFLOW);
    this._ensure(st.lim = st.base + payload_sz);
    this.buf.set(pkt.subarray(HDR_SZ, HDR_SZ + payload_sz), st.base);
  }

  private _branch(node: BranchNode, st: EncodingState): void {
    const { val: children } = node;
    if (children === null) return;

    st.base = ALIGN(st.base, WORD_SZ);
    const table_start: u32 = st.base + WORD_SZ;

    const len: u32 = children.length;
    if (len > (this.max_cap >>> 3)) throw new MarshalError(Errno.EOVERFLOW);

    this._ensure(st.lim = table_start + len * WORD_SZ);

    SET_WORD(this.buf_view, st.base, len, RESERVED);

    let table_off: u32 = table_start;
    const child_st: EncodingState = { base: st.lim, lim: st.lim, type: SpecialType.NIL };
    for (const child of children) {
      child_st.base = child_st.lim;
      child_st.type = child.type;

      this._node(child, child_st);
      const rel_off = child_st.base === child_st.lim ? 0 : child_st.base - st.base;

      SET_WORD(this.buf_view, table_off, rel_off, FIELD_SET(0, DESC_TYPE_SHAMT, DESC_TYPE_MASK, child_st.type));

      table_off += WORD_SZ;
    }
    this._ensure(st.lim = ALIGN(child_st.lim, WORD_SZ));
  }

  private _scalar(node: ScalarNode, st: EncodingState): void {
    const { type, val } = node;
    if (val === null) return;

    let elem_sz: AlignUnit;
    let set_payload: (off: u32) => void;

    switch (type) {
      case ScalarType.U8:
        elem_sz = 1;
        set_payload = (off: u32): void => this.buf_view.setUint8(off, val as u8);
        break;
      case ScalarType.I8:
        elem_sz = 1;
        set_payload = (off: u32): void => this.buf_view.setInt8(off, val as i8);
        break;
      case ScalarType.U16:
        elem_sz = 2;
        set_payload = (off: u32): void => this.buf_view.setUint16(off, val as u16, LE);
        break;
      case ScalarType.I16:
        elem_sz = 2;
        set_payload = (off: u32): void => this.buf_view.setInt16(off, val as i16, LE);
        break;
      case ScalarType.U32:
        elem_sz = 4;
        set_payload = (off: u32): void => this.buf_view.setUint32(off, val as u32, LE);
        break;
      case ScalarType.I32:
        elem_sz = 4;
        set_payload = (off: u32): void => this.buf_view.setInt32(off, val as i32, LE);
        break;
      case ScalarType.U64:
        elem_sz = 8;
        set_payload = (off: u32): void => this.buf_view.setBigUint64(off, val as u64, LE);
        break;
      case ScalarType.I64:
        elem_sz = 8;
        set_payload = (off: u32): void => this.buf_view.setBigInt64(off, val as i64, LE);
        break;
      case ScalarType.F32:
        elem_sz = 4;
        set_payload = (off: u32): void => this.buf_view.setFloat32(off, val as f32, LE);
        break;
      case ScalarType.F64:
        elem_sz = 8;
        set_payload = (off: u32): void => this.buf_view.setFloat64(off, val as f64, LE);
        break;

      case ScalarType.BOOL:
        elem_sz = 1;
        set_payload = (off: u32): void => this.buf_view.setUint8(off, BOOL(val));
        break;

      default:
        throw new MarshalError(Errno.EBADMSG);
    }

    st.base = ALIGN(st.base, elem_sz);

    this._ensure(st.lim = st.base + elem_sz);
    set_payload(st.base);
  }

  private _seq(node: SequenceNode, st: EncodingState): void {
    const { type, val } = node;
    if (val === null) return;

    st.base = ALIGN(st.base, WORD_SZ);
    const payload_start: u32 = st.base + WORD_SZ;

    let later: boolean = false;

    let len: u32 = 0;
    let payload_sz: u32 = 0;
    let set_payload: () => void;

    switch (type) {
      case SequenceType.U8_ARRAY:
      case SequenceType.I8_ARRAY:
      case SequenceType.U16_ARRAY:
      case SequenceType.I16_ARRAY:
      case SequenceType.U32_ARRAY:
      case SequenceType.I32_ARRAY:
      case SequenceType.U64_ARRAY:
      case SequenceType.I64_ARRAY:
      case SequenceType.F32_ARRAY:
      case SequenceType.F64_ARRAY: {
        const arr = val as TypedArray | BigTypedArray;
        len = arr.length;
        payload_sz = len * arr.BYTES_PER_ELEMENT;

        set_payload = (): void => this.buf.set(new Uint8Array(arr.buffer, arr.byteOffset, arr.byteLength), payload_start);
        break;
      }

      case SequenceType.BITSET: {
        const arr = val as U8Array;
        len = arr.length;
        payload_sz = Math.floor((len + 7) / 8);

        // bit packing: bit i -> byte[i >> 3], bit position (i & 7), LSB-first within each byte
        set_payload = (): void => {
          this.buf.fill(0, payload_start, payload_start + payload_sz);

          for (let i = 0; i < len; i++) {
            if (arr[i]) {
              this.buf[payload_start + (i >>> 3)] |= 1 << (i & 7);
            }
          }
        };
        break;
      }

      // encodeInto() the free tail first, and on overflow reserve the worst case (3 bytes a UTF-16 unit) and retry once.
      case SequenceType.STR: {
        const s = val as string;

        later = true;
        set_payload = (): void => {
          const min_needed: u32 = s.length;
          if (min_needed > this.max_cap - payload_start) throw new MarshalError(Errno.EOVERFLOW);

          if (this.buf.byteLength - payload_start < min_needed) {
            this._ensure(payload_start + min_needed);
          }

          let view = this.buf.subarray(payload_start);
          let res = encoding.te.encodeInto(s, view);

          if (res.read < s.length) {
            const payload_lim: u32 = payload_start + Math.min(3 * s.length, this.max_cap - payload_start);
            this._ensure(payload_lim);

            view = this.buf.subarray(payload_start, payload_lim);
            res = encoding.te.encodeInto(s, view);
            if (res.read < s.length) throw new MarshalError(Errno.EOVERFLOW);
          }

          len = res.written;
          payload_sz = len + 1; // wire payload includes trailing NUL for C-friendly reads

          this._ensure(payload_start + payload_sz);
          this.buf[payload_start + len] = 0;
        };
        break;
      }

      // `len` + 1 offsets into the UTF-8 after them, then that UTF-8.
      case SequenceType.STRS: {
        const items = (val as string[]).map((s) => encoding.te.encode(s));
        len = items.length;
        const lane: u32 = 4 * (len + 1);
        payload_sz = lane + items.reduce((n, b) => n + b.byteLength, 0);
        set_payload = (): void => {
          let off: u32 = 0;
          this.buf_view.setUint32(payload_start, 0, true);
          items.forEach((b, i) => {
            this.buf.set(b, payload_start + lane + off);
            off += b.byteLength;
            this.buf_view.setUint32(payload_start + 4 * (i + 1), off, true);
          });
        };
        break;
      }

      default:
        throw new MarshalError(Errno.EBADMSG);
    }

    // set_payload may grow and replace buf_view, so the len word is written after it.
    if (later) {
      set_payload();
      this._ensure(st.lim = payload_start + ALIGN(payload_sz, WORD_SZ));
    } else {
      if (payload_sz > this.max_cap - payload_start) throw new MarshalError(Errno.EOVERFLOW);
      this._ensure(st.lim = payload_start + ALIGN(payload_sz, WORD_SZ));
      set_payload();
    }
    SET_WORD(this.buf_view, st.base, len, RESERVED);
  }
}

export const encoder = new TreeEncoder();

/** Where a packet is written in place: a writer begun over the room for a
 * root branch of `count` children. */
export interface PacketRoom {
  writer(count: u32): PacketWriter;
}

/**
 * Writes a root-branch packet of scalars and strings in place, into bytes the
 * caller holds, byte for byte as `TreeEncoder` would and with no tree built
 * first: rust/kit/src/wire/xtp/encode.rs `PacketWriter`, in the shapes the
 * host writes. A packet declares its child count up front (`begin`); a child
 * that does not fit makes `finish` refuse, and a miscount throws. One writer
 * serves every packet its room hands out, begun again for each.
 */
export class PacketWriter {
  private bytes: U8Array = new Uint8Array(0);
  private view: DataView = new DataView(new ArrayBuffer(0));
  /** Where the packet begins in `bytes`, and the bytes it may take. */
  start: u32 = 0;
  private cap: u32 = 0;
  private count: u32 = 0;
  private written: u32 = 0;
  /** The packet's end so far, from `start`. */
  private lim: u32 = 0;
  private fits: boolean = false;

  /** `bytes` and `view` see the same memory from the same offset. */
  begin(bytes: U8Array, view: DataView, start: u32, cap: u32, count: u32): this {
    this.bytes = bytes;
    this.view = view;
    this.start = start;
    this.cap = cap;
    this.count = count;
    this.written = 0;
    const end: u32 = HDR_SZ + WORD_SZ * (1 + count);
    this.fits = end <= cap;
    this.lim = this.fits ? end : HDR_SZ;
    if (this.fits) {
      bytes.fill(0, start, start + end);
      SET_WORD(view, start, MAGIC, RESERVED);
      SET_WORD(view, start + HDR_SZ, count, RESERVED);
    }
    return this;
  }

  /** Where `bytes` aligned to `unit` begin, zeroed from the end so far; -1 past the room. */
  private reserve(unit: AlignUnit, bytes: u32): number {
    if (!this.fits) return -1;
    const base: u32 = ALIGN(this.lim, unit);
    const end: u32 = base + bytes;
    if (end > this.cap) {
      this.fits = false;
      return -1;
    }
    this.bytes.fill(0, this.start + this.lim, this.start + end);
    this.lim = end;
    return base;
  }

  private entry(base: number, type: NodeType): this {
    if (this.written >= this.count) throw new InvariantError(Errno.ENOSPC);
    if (this.fits) {
      const rel_off: u32 = base < 0 ? 0 : base - HDR_SZ;
      SET_WORD(this.view, this.start + HDR_SZ + WORD_SZ * (1 + this.written), rel_off, FIELD_SET(0, DESC_TYPE_SHAMT, DESC_TYPE_MASK, type));
    }
    this.written++;
    return this;
  }

  u8(v: u8 | null): this {
    const base = v === null ? -1 : this.reserve(1, 1);
    if (base >= 0) this.view.setUint8(this.start + base, v as u8);
    return this.entry(base, ScalarType.U8);
  }

  u32(v: u32 | null): this {
    const base = v === null ? -1 : this.reserve(4, 4);
    if (base >= 0) this.view.setUint32(this.start + base, v as u32, LE);
    return this.entry(base, ScalarType.U32);
  }

  i32(v: i32 | null): this {
    const base = v === null ? -1 : this.reserve(4, 4);
    if (base >= 0) this.view.setInt32(this.start + base, v as i32, LE);
    return this.entry(base, ScalarType.I32);
  }

  f64(v: f64 | null): this {
    const base = v === null ? -1 : this.reserve(8, 8);
    if (base >= 0) this.view.setFloat64(this.start + base, v as f64, LE);
    return this.entry(base, ScalarType.F64);
  }

  bool(v: boolean | null): this {
    const base = v === null ? -1 : this.reserve(1, 1);
    if (base >= 0) this.view.setUint8(this.start + base, BOOL(v));
    return this.entry(base, ScalarType.BOOL);
  }

  /** UTF-8 as `TextEncoder` writes it, a NUL after it that its length leaves out. */
  str(s: string | null): this {
    if (s === null || !this.fits) return this.entry(-1, SequenceType.STR);
    const base: u32 = ALIGN(this.lim, WORD_SZ);
    const payload: u32 = base + WORD_SZ;
    const res = payload < this.cap ? encoding.te.encodeInto(s, this.bytes.subarray(this.start + payload, this.start + this.cap)) : null;
    const end: u32 = res === null ? 0 : payload + ALIGN(res.written + 1, WORD_SZ);
    if (res === null || res.read < s.length || end > this.cap) {
      this.fits = false;
      return this.entry(-1, SequenceType.STR);
    }
    this.bytes.fill(0, this.start + this.lim, this.start + payload);
    this.bytes.fill(0, this.start + payload + res.written, this.start + end);
    SET_WORD(this.view, this.start + base, res.written, RESERVED);
    this.lim = end;
    return this.entry(base, SequenceType.STR);
  }

  /** The packet's length, or -1 when a child did not fit. */
  finish(): number {
    if (this.written !== this.count) throw new InvariantError(Errno.ENOTRECOVERABLE);
    if (!this.fits) return -1;
    const end: u32 = ALIGN(this.lim, WORD_SZ);
    if (end > this.cap) return -1;
    this.bytes.fill(0, this.start + this.lim, this.start + end);
    SET_WORD(this.view, this.start + WORD_SZ, end - HDR_SZ, FIELD_SET(0, DESC_TYPE_SHAMT, DESC_TYPE_MASK, SpecialType.BRANCH));
    return end;
  }
}
