// @xpute/link/cabi.ts

/** A packet's values in the Component Model's Canonical ABI layout, as
 * xpute-link's cabi.rs lays them: each at its type's alignment,
 * little-endian, a string's or a list's bytes past the packet's fixed part,
 * their pointer an offset from the packet's start. A packet that does not
 * hold what its type says is refused (EBADMSG). */

import {
  type BigTypedArray,
  to_bytes,
  type TypedArray,
  type U8Array,
} from "@xpute/kit/abi/array.ts";
import { te } from "@xpute/kit/codec/encoding.ts";
import { ERRNO_EBADMSG } from "@xpute/kit/status/errno.spec.ts";
import { XputeError } from "@xpute/kit/status/error.ts";

const align_to = (at: number, align: number): number =>
  Math.ceil(at / align) * align;

const UTF8 = new TextDecoder("utf-8", { fatal: true });

/** Writes a packet into the bytes it is given: its fixed part first
 * (`start`), then its strings' and lists' bytes. A packet past them spoils,
 * and `finish` answers -1; a writer nothing was written by answers 0, no
 * packet. One writer is aimed at packet after packet (`reset`), and makes
 * views of a buffer only when it is aimed at another, so a side writes each
 * packet in place with nothing made per packet. */
export class Lower {
  /** The buffer the packet lies in, whole, and the views over it. */
  private mem!: ArrayBufferLike;
  private buf!: U8Array<ArrayBufferLike>;
  private view!: DataView<ArrayBufferLike>;
  /** Where the packet begins in the buffer, and the most it may hold. */
  private off = 0;
  private cap = 0;
  private end = 0;
  private overflow = false;

  /** A packet of a fixed part `fixed_sz` long, zeroed, in `buf`. */
  constructor(buf: U8Array<ArrayBufferLike>, fixed_sz: number) {
    this.reset(buf.buffer, buf.byteOffset, buf.length);
    this.start(fixed_sz);
  }

  /** Aims the writer at the `cap` bytes at `off` in `mem`, nothing written
   * there yet, in place of the packet it wrote before. */
  reset(mem: ArrayBufferLike, off: number, cap: number): void {
    if (mem !== this.mem) {
      this.mem = mem;
      this.buf = new Uint8Array(mem);
      this.view = new DataView(mem);
    }
    this.off = off;
    this.cap = cap;
    this.overflow = false;
    this.end = 0;
  }

  /** Starts the packet with a fixed part `fixed_sz` long, zeroed. */
  start(fixed_sz: number): void {
    this.overflow = fixed_sz > this.cap;
    if (!this.overflow) this.buf.fill(0, this.off, this.off + fixed_sz);
    this.end = fixed_sz;
  }

  /** Where the `n` bytes at `at` lie in the buffer, or -1 where they run past
   * the packet's bytes, which spoils it. */
  private within(at: number, n: number): number {
    if (this.overflow || at + n > this.cap) this.overflow = true;
    return this.overflow ? -1 : this.off + at;
  }

  bool(at: number, v: boolean): void {
    const o = this.within(at, 1);
    if (o >= 0) this.view.setUint8(o, v ? 1 : 0);
  }
  u8(at: number, v: number): void {
    const o = this.within(at, 1);
    if (o >= 0) this.view.setUint8(o, v);
  }
  i8(at: number, v: number): void {
    const o = this.within(at, 1);
    if (o >= 0) this.view.setInt8(o, v);
  }
  u16(at: number, v: number): void {
    const o = this.within(at, 2);
    if (o >= 0) this.view.setUint16(o, v, true);
  }
  i16(at: number, v: number): void {
    const o = this.within(at, 2);
    if (o >= 0) this.view.setInt16(o, v, true);
  }
  u32(at: number, v: number): void {
    const o = this.within(at, 4);
    if (o >= 0) this.view.setUint32(o, v, true);
  }
  i32(at: number, v: number): void {
    const o = this.within(at, 4);
    if (o >= 0) this.view.setInt32(o, v, true);
  }
  u64(at: number, v: bigint): void {
    const o = this.within(at, 8);
    if (o >= 0) this.view.setBigUint64(o, v, true);
  }
  i64(at: number, v: bigint): void {
    const o = this.within(at, 8);
    if (o >= 0) this.view.setBigInt64(o, v, true);
  }
  f32(at: number, v: number): void {
    const o = this.within(at, 4);
    if (o >= 0) this.view.setFloat32(o, v, true);
  }
  f64(at: number, v: number): void {
    const o = this.within(at, 8);
    if (o >= 0) this.view.setFloat64(o, v, true);
  }

  /** Takes `len` elements of `sz` bytes at `align` past what is written,
   * zeroed, and puts their offset and count at `at`. It answers where they
   * begin, or null where they run past the bytes it is given. */
  list(at: number, len: number, sz: number, align: number): number | null {
    const base = align_to(this.end, align);
    if (this.within(base, len * sz) < 0) return null;
    this.buf.fill(0, this.off + this.end, this.off + base + len * sz);
    this.end = base + len * sz;
    this.u32(at, base);
    this.u32(at + 4, len);
    return base;
  }

  /** Puts `a`'s bytes past what is written, at its elements' alignment, and
   * their offset and count at `at`. */
  arr(at: number, a: TypedArray | BigTypedArray): void {
    const base = this.list(
      at,
      a.length,
      a.BYTES_PER_ELEMENT,
      a.BYTES_PER_ELEMENT,
    );
    if (base !== null) this.buf.set(to_bytes(a), this.off + base);
  }

  str(at: number, s: string): void {
    const b = te.encode(s);
    const base = this.list(at, b.length, 1, 1);
    if (base !== null) this.buf.set(b, this.off + base);
  }

  /** The packet's length, or -1 where it is past the bytes it is given. */
  finish(): number {
    return this.overflow ? -1 : this.end;
  }
}

/** Reads a packet's values where its type says they lie, refusing EBADMSG
 * what lies past the packet, a misaligned list, or a string not UTF-8. One
 * reader is aimed at packet after packet (`reset`), as a writer is, so a side
 * reads each method's args in place. */
export class Lift {
  /** The buffer the packet lies in, whole, and the views over it. */
  private mem!: ArrayBuffer;
  private buf!: U8Array;
  private view!: DataView;
  /** Where the packet begins in the buffer, and its length. */
  private off = 0;
  private len = 0;

  constructor(pkt: U8Array) {
    this.reset(pkt.buffer, pkt.byteOffset, pkt.length);
  }

  /** Aims the reader at the `len` bytes at `off` in `mem`, in place of the
   * packet it read before. */
  reset(mem: ArrayBuffer, off: number, len: number): void {
    if (mem !== this.mem) {
      this.mem = mem;
      this.buf = new Uint8Array(mem);
      this.view = new DataView(mem);
    }
    this.off = off;
    this.len = len;
  }

  /** The packet's bytes, the receiver's for the crossing alone, or null for
   * no packet. */
  pkt(): U8Array | null {
    if (this.len === 0) return null;
    return this.buf.subarray(this.off, this.off + this.len);
  }

  /** Where the `n` bytes at `at` lie in the buffer, refused where they run
   * past the packet. */
  private within(at: number, n: number): number {
    if (at + n > this.len) throw new XputeError(ERRNO_EBADMSG);
    return this.off + at;
  }

  /** A bool: 0 is false and any other byte true, as the ABI lifts it. */
  bool(at: number): boolean {
    return this.view.getUint8(this.within(at, 1)) !== 0;
  }
  u8(at: number): number {
    return this.view.getUint8(this.within(at, 1));
  }
  i8(at: number): number {
    return this.view.getInt8(this.within(at, 1));
  }
  u16(at: number): number {
    return this.view.getUint16(this.within(at, 2), true);
  }
  i16(at: number): number {
    return this.view.getInt16(this.within(at, 2), true);
  }
  u32(at: number): number {
    return this.view.getUint32(this.within(at, 4), true);
  }
  i32(at: number): number {
    return this.view.getInt32(this.within(at, 4), true);
  }
  u64(at: number): bigint {
    return this.view.getBigUint64(this.within(at, 8), true);
  }
  i64(at: number): bigint {
    return this.view.getBigInt64(this.within(at, 8), true);
  }
  f32(at: number): number {
    return this.view.getFloat32(this.within(at, 4), true);
  }
  f64(at: number): number {
    return this.view.getFloat64(this.within(at, 8), true);
  }

  /** A discriminant of `n` cases at `at`, refused where it names none. */
  case(at: number, n: number): number {
    const d = n <= 256 ? this.u8(at) : n <= 65536 ? this.u16(at) : this.u32(at);
    if (d >= n) throw new XputeError(ERRNO_EBADMSG);
    return d;
  }

  /** The elements of `sz` bytes at `align` whose offset and count lie at
   * `at`: where they begin, and how many. */
  list(at: number, sz: number, align: number): readonly [number, number] {
    const base = this.u32(at);
    const len = this.u32(at + 4);
    if (base % align !== 0 || base + len * sz > this.len) {
      throw new XputeError(ERRNO_EBADMSG);
    }
    return [base, len];
  }

  /** Each element of a list at `at`, read by `get` at its offset. */
  map<T>(at: number, sz: number, align: number, get: (at: number) => T): T[] {
    const [base, len] = this.list(at, sz, align);
    return Array.from({ length: len }, (_, i) => get(base + i * sz));
  }

  /** A list of numbers at `at`, copied out as `ctor`'s typed array. */
  arr<A>(
    at: number,
    ctor: {
      new (b: ArrayBuffer): A;
      readonly BYTES_PER_ELEMENT: number;
    },
  ): A {
    const n = ctor.BYTES_PER_ELEMENT;
    const [base, len] = this.list(at, n, n);
    const from = this.off + base;
    return new ctor(this.buf.slice(from, from + len * n).buffer);
  }

  private bytes(at: number): U8Array {
    const [base, len] = this.list(at, 1, 1);
    return this.buf.subarray(this.off + base, this.off + base + len);
  }

  str(at: number): string {
    try {
      return UTF8.decode(this.bytes(at));
    } catch {
      throw new XputeError(ERRNO_EBADMSG);
    }
  }
}
