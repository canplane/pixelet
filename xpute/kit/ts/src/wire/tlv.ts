// @xpute/kit/wire/tlv.ts

/**
 * TLV: self-delimiting binary field stream.
 *
 * Scope
 * -----
 * - This module encodes and decodes a generic sequential TLV stream.
 * - It is intentionally schema-light: meaning is assigned by higher layers.
 * - The stream is self-delimiting via END tag.
 * - This format is sequential and stream-oriented, not a relocatable object layout.
 *
 * Wire Layout
 * -----------
 * Repeated sequence of:
 *   [tag | payload]
 * terminated by:
 *   [END]
 *
 * Tag domain:
 * - u8 tag values
 * - tag space is format-owned
 * - END is a control tag, not a value payload kind
 *
 * Payload Layout
 * --------------
 * - BOOL  = [u8]
 * - U8    = [u8]
 * - I8    = [i8]
 * - U16   = [u16]
 * - I16   = [i16]
 * - U32   = [u32]
 * - I32   = [i32]
 * - U64   = [u64]
 * - I64   = [i64]
 * - F32   = [f32]
 * - F64   = [f64]
 * - STR   = [u32 len][utf8 bytes]
 * - BYTES = [u32 len][raw bytes]
 *
 * Endianness
 * ----------
 * - All scalar payloads use little-endian encoding via scalar.ts.
 *
 * Stream Semantics
 * ----------------
 * - END terminates the logical stream.
 * - Bytes after END are ignored by iteration semantics.
 * - Missing END is tolerated only if the buffer ends exactly after the last full element;
 *   truncation during element decode is an error.
 * - end() tells which it was: closed by END, or unclosed.
 * - Elements are interpreted strictly in forward stream order.
 * - A STR that is not UTF-8 reads with U+FFFD in place of each bad sequence.
 *
 * Reader / Writer Contract
 * ------------------------
 * - TlvWriter is append-only until finish(); write-after-finish is invalid.
 * - finish() appends END exactly once.
 * - TlvReader iterates elements sequentially from the start of the buffer.
 * - Unknown tag values are rejected with EBADMSG.
 * - Truncated payloads are rejected with EBADMSG.
 *
 * Value Model
 * -----------
 * - TLV is a stream format, not a random-access object layout.
 * - Repeated values are allowed.
 * - Field names, uniqueness, ordering requirements, and semantic constraints
 *   are entirely owned by higher layers.
 *
 * Non-Goals
 * ---------
 * - No schema, field table, or offset index.
 * - No nested container protocol in this base format.
 * - No checksum/hash/compression/encryption inside this format.
 * - No canonical integer width normalization beyond the explicit tag written.
 */

import type {
  f32,
  f64,
  i16,
  i32,
  i64,
  i8,
  primitive,
  u16,
  u32,
  u64,
  u8,
} from "@xpute/kit/abi/word.ts";
import {
  I32_MAX,
  I32_MIN,
  I64_MAX,
  I64_MIN,
  U16_SZ,
  U32_SZ,
  U64_SZ,
  U8_SZ,
} from "@xpute/kit/abi/word.ts";
import type { ByteArray } from "@xpute/kit/abi/array.ts";
import * as encoding from "@xpute/kit/codec/encoding.ts";
import type { Stream } from "@xpute/kit/wire/scalar.ts";
import * as scalar from "@xpute/kit/wire/scalar.ts";
import { ERRNO_EBADMSG } from "@xpute/kit/status/errno.spec.ts";
import { XputeError } from "@xpute/kit/status/error.ts";

// ============ ABI ============

// ---- Elements ----

// TLV tag domain includes both value tags and control tags (e.g. END)
const Tag = {
  END: 0x00, // END is a stream terminator control tag, not a payload element kind.

  BOOL: 0x01, // [u8 0|1]

  U8: 0x02,
  I8: 0x03,
  U16: 0x04,
  I16: 0x05,
  U32: 0x06,
  I32: 0x07,
  U64: 0x08,
  I64: 0x09,
  F32: 0x0a,
  F64: 0x0b,

  STR: 0x10, // [u32 len][utf8 bytes]

  BYTES: 0x20, // [u32 len][raw bytes], opaque binary blob (nonce/token/key/u8 payload)
} as const;
type Tag = (typeof Tag)[keyof typeof Tag];

export type TlvValue = primitive | ByteArray;

// ============ Tag I/O ============

// tag I/O is TLV-specific (not general scalar)
const put_tag = (s: Stream, tag: Tag): void => scalar.putu8(s, tag);
const get_tag = (s: Stream): u8 => scalar.getu8(s);

// ============ Writer ============

// ---- Writer core ----

export class TlvWriter {
  private _buf: ByteArray;
  private _stream: Stream;
  private _sealed: boolean = false;

  constructor(init_cap: u32 = 1024) {
    this._buf = new Uint8Array(init_cap);
    this._stream = scalar.stream(this._buf);
  }

  private _ensure(sz: u32) {
    if (this._stream.off + sz > this._buf.byteLength) {
      const new_cap: u32 = Math.max(
        this._buf.byteLength * 2,
        this._stream.off + sz,
      );
      const new_buf: ByteArray = new Uint8Array(new_cap);
      new_buf.set(this._buf);
      this._buf = new_buf;

      // IMPORTANT: stream must be rebound to the new backing buffer.
      this._stream.buf = this._buf;
      this._stream.view = new DataView(
        this._buf.buffer,
        this._buf.byteOffset,
        this._buf.byteLength,
      );
      // offset stays valid (same numeric off)
    }
  }

  data(): ByteArray {
    return this._buf.subarray(0, this._stream.off);
  }

  /** Seal stream by appending END. Writer must not be used after this. */
  finish(): ByteArray {
    if (!this._sealed) {
      this._ensure(1);
      put_tag(this._stream, Tag.END);
      this._sealed = true;
    }
    return this.data();
  }

  private _check_unsealed() {
    if (this._sealed) {
      throw new XputeError(ERRNO_EBADMSG);
    }
  }

  // ---- Primitive writers ----

  u8(x: u8): this {
    this._check_unsealed();
    this._ensure(1 + U8_SZ);
    put_tag(this._stream, Tag.U8);
    scalar.putu8(this._stream, x);
    return this;
  }
  i8(x: i8): this {
    this._check_unsealed();
    this._ensure(1 + U8_SZ);
    put_tag(this._stream, Tag.I8);
    scalar.puti8(this._stream, x);
    return this;
  }
  u16(x: u16): this {
    this._check_unsealed();
    this._ensure(1 + U16_SZ);
    put_tag(this._stream, Tag.U16);
    scalar.putu16(this._stream, x);
    return this;
  }
  i16(x: i16): this {
    this._check_unsealed();
    this._ensure(1 + U16_SZ);
    put_tag(this._stream, Tag.I16);
    scalar.puti16(this._stream, x);
    return this;
  }
  u32(x: u32): this {
    this._check_unsealed();
    this._ensure(1 + U32_SZ);
    put_tag(this._stream, Tag.U32);
    scalar.putu32(this._stream, x);
    return this;
  }
  i32(x: i32): this {
    this._check_unsealed();
    this._ensure(1 + U32_SZ);
    put_tag(this._stream, Tag.I32);
    scalar.puti32(this._stream, x);
    return this;
  }
  u64(x: u64): this {
    this._check_unsealed();
    this._ensure(1 + U64_SZ);
    put_tag(this._stream, Tag.U64);
    scalar.putu64(this._stream, x);
    return this;
  }
  i64(x: i64): this {
    this._check_unsealed();
    this._ensure(1 + U64_SZ);
    put_tag(this._stream, Tag.I64);
    scalar.puti64(this._stream, x);
    return this;
  }
  f32(x: f32): this {
    this._check_unsealed();
    this._ensure(1 + U32_SZ);
    put_tag(this._stream, Tag.F32);
    scalar.putf32(this._stream, x);
    return this;
  }
  f64(x: f64): this {
    this._check_unsealed();
    this._ensure(1 + U64_SZ);
    put_tag(this._stream, Tag.F64);
    scalar.putf64(this._stream, x);
    return this;
  }

  bool(x: boolean): this {
    this._check_unsealed();
    this._ensure(1 + U8_SZ);
    put_tag(this._stream, Tag.BOOL);
    scalar.putu8(this._stream, x ? 1 : 0);
    return this;
  }

  // ---- Ref writers ----

  str(s: string): this {
    this._check_unsealed();
    const data: ByteArray = encoding.te.encode(s);
    const len: u32 = data.length;
    this._ensure(1 + U32_SZ + len);
    put_tag(this._stream, Tag.STR);
    scalar.putu32(this._stream, len);
    this._buf.set(data, this._stream.off);
    this._stream.off += len;
    return this;
  }

  bytes(b: ByteArray): this {
    this._check_unsealed();
    const len: u32 = b.byteLength;
    this._ensure(1 + U32_SZ + len);
    put_tag(this._stream, Tag.BYTES);
    scalar.putu32(this._stream, len);
    this._buf.set(b, this._stream.off);
    this._stream.off += len;
    return this;
  }

  // ---- Convenience writers ----

  // Auto-dispatch helper:
  // - string -> STR
  // - Uint8Array -> BYTES
  // - bigint/number -> explicit integer/float tags by runtime shape/range
  write_all(vals: TlvValue[]): this {
    for (const v of vals) {
      switch (typeof v) {
        case "boolean":
          this.bool(v);
          break;
        case "number":
          if (Number.isSafeInteger(v)) {
            if (v >= I32_MIN && v <= I32_MAX) this.i32(v);
            else this.i64(BigInt(v));
          } else {
            // float path (includes NaN/Inf)
            this.f64(v);
          }
          break;
        case "bigint":
          if (v < I64_MIN || v > I64_MAX) {
            throw new XputeError(ERRNO_EBADMSG);
          }
          this.i64(v);
          break;
        case "string":
          this.str(v);
          break;
        default:
          this.bytes(v);
      }
    }
    return this;
  }
}

// ============ Reader ============

/** A sequence stopped without a fault: END was read, or the input ran out
 * between two values. */
export type TlvEnd = "closed" | "unclosed";

// ---- Reader core ----

export class TlvReader implements Iterable<TlvValue> {
  private _stream: Stream;
  private _end: u32;
  private _ending: TlvEnd | null = null;

  private readonly _buf: ByteArray;

  constructor(_buf: ByteArray) {
    this._buf = _buf;
    this._stream = scalar.stream(_buf);
    this._end = _buf.byteLength;
  }

  /** How the sequence ended, once the reader has reached its end; null while
   * values remain, and after a fault. */
  end(): TlvEnd | null {
    return this._ending;
  }

  /** Materialize the remaining stream into an array, up to END or the end of the input. */
  read_all(): TlvValue[] {
    const out: TlvValue[] = [];
    for (const v of this) out.push(v);
    return out;
  }

  *[Symbol.iterator](): Iterator<TlvValue> {
    while (this._ending === null) {
      if (this._stream.off >= this._end) {
        this._ending = "unclosed";
        break;
      }
      const tag = get_tag(this._stream);
      if (tag === Tag.END) {
        this._ending = "closed";
        break;
      }

      switch (tag) {
        case Tag.U8:
          yield this._u8();
          break;
        case Tag.I8:
          yield this._i8();
          break;
        case Tag.U16:
          yield this._u16();
          break;
        case Tag.I16:
          yield this._i16();
          break;
        case Tag.U32:
          yield this._u32();
          break;
        case Tag.I32:
          yield this._i32();
          break;
        case Tag.U64:
          yield this._u64();
          break;
        case Tag.I64:
          yield this._i64();
          break;
        case Tag.F32:
          yield this._f32();
          break;
        case Tag.F64:
          yield this._f64();
          break;
        case Tag.BOOL:
          yield this._bool();
          break;
        case Tag.STR:
          yield this._str();
          break;
        case Tag.BYTES:
          yield this._bytes();
          break;
        default:
          throw new XputeError(ERRNO_EBADMSG);
      }
    }
  }

  // ---- Reader bounds ----

  private _need(n: u32): void {
    if (this._stream.off + n > this._end) throw new XputeError(ERRNO_EBADMSG);
  }

  // ---- Primitive readers ----

  private _u8(): u8 {
    this._need(U8_SZ);
    return scalar.getu8(this._stream);
  }
  private _i8(): i8 {
    this._need(U8_SZ);
    return scalar.geti8(this._stream);
  }
  private _u16(): u16 {
    this._need(U16_SZ);
    return scalar.getu16(this._stream);
  }
  private _i16(): i16 {
    this._need(U16_SZ);
    return scalar.geti16(this._stream);
  }
  private _u32(): u32 {
    this._need(U32_SZ);
    return scalar.getu32(this._stream);
  }
  private _i32(): i32 {
    this._need(U32_SZ);
    return scalar.geti32(this._stream);
  }
  private _u64(): u64 {
    this._need(U64_SZ);
    return scalar.getu64(this._stream);
  }
  private _i64(): i64 {
    this._need(U64_SZ);
    return scalar.geti64(this._stream);
  }
  private _f32(): f32 {
    this._need(U32_SZ);
    return scalar.getf32(this._stream);
  }
  private _f64(): f64 {
    this._need(U64_SZ);
    return scalar.getf64(this._stream);
  }

  private _bool(): boolean {
    this._need(U8_SZ);
    return scalar.getu8(this._stream) !== 0;
  }

  // ---- Ref readers ----

  private _str(): string {
    const len = this._u32();
    this._need(len);
    const s: string = encoding.td.decode(
      this._buf.subarray(this._stream.off, this._stream.off + len),
    );
    this._stream.off += len;
    return s;
  }

  private _bytes(): ByteArray {
    const len = this._u32();
    this._need(len);
    const b = this._buf.subarray(this._stream.off, this._stream.off + len);
    this._stream.off += len;
    return b;
  }
}
