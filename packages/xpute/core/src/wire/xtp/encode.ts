// @xpute/core/wire/xtp/encode.ts

import { BOOL, type f32, type f64, FIELD_GET, FIELD_SET, type i16, type i32, type i64, type i8, type u16, type u32, type u64, type u8 } from "@xpute/core/abi/word.ts";
import type { BigTypedArray, TypedArray, U8Array } from "@xpute/core/abi/array.ts";
import * as encoding from "@xpute/core/codec/encoding.ts";
import { Errno } from "@xpute/core/status/errno.spec.ts";
import { MarshalError } from "@xpute/core/status/error.ts";

import type { AlignUnit, BranchNode, EncodingState, GraftNode, Node, NodeType, ScalarNode, SequenceNode } from "./spec.ts";
import {
  ALIGN,
  ALIGN_SZ,
  DESC_TYPE_MASK,
  DESC_TYPE_SHAMT,
  GET_WORD,
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

const WORD_REG: [u32, u32] = [NONE, NONE];

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

    const [magic] = GET_WORD(hdr_view, 0, WORD_REG);
    if (magic !== MAGIC) throw new MarshalError(Errno.EBADMSG);

    const [payload_sz, desc] = GET_WORD(hdr_view, WORD_SZ, WORD_REG);
    if (payload_sz > pkt.byteLength - HDR_SZ) throw new MarshalError(Errno.EBADMSG);
    st.type = FIELD_GET(desc, DESC_TYPE_SHAMT, DESC_TYPE_MASK) as NodeType;

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

      // encodeInto() the free tail first, and on overflow reserve the worst case (4 bytes a UTF-16 unit) and retry once.
      case SequenceType.STR: {
        const s = val as string;

        later = true;
        set_payload = (): void => {
          const min_needed: u32 = s.length;

          if (s.length > ((this.max_cap - payload_start) >>> 2)) {
            throw new MarshalError(Errno.EOVERFLOW);
          }
          const max_needed: u32 = s.length * 4;

          if (this.buf.byteLength - payload_start < min_needed) {
            this._ensure(payload_start + min_needed);
          }

          let view = this.buf.subarray(payload_start);
          let res = encoding.te.encodeInto(s, view);

          if (res.read < s.length) {
            const payload_lim: u32 = payload_start + max_needed;
            this._ensure(payload_lim);

            view = this.buf.subarray(payload_start, payload_lim);
            res = encoding.te.encodeInto(s, view);
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
