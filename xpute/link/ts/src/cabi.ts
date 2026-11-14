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

/** Writes a packet into `buf`: its fixed part first, then its strings' and
 * lists' bytes. A packet past `buf` spoils, and `finish` answers -1. */
export class Lower {
  private readonly buf: U8Array;
  private readonly view: DataView;
  private end: number;
  private overflow: boolean;

  /** A packet of a fixed part `fixed_sz` long, zeroed, in `buf`. */
  constructor(buf: U8Array, fixed_sz: number) {
    this.buf = buf;
    this.view = new DataView(buf.buffer, buf.byteOffset, buf.byteLength);
    this.overflow = fixed_sz > buf.length;
    if (!this.overflow) buf.fill(0, 0, fixed_sz);
    this.end = fixed_sz;
  }

  private fits(at: number, n: number): boolean {
    if (this.overflow || at + n > this.buf.length) this.overflow = true;
    return !this.overflow;
  }

  bool(at: number, v: boolean): void {
    if (this.fits(at, 1)) this.view.setUint8(at, v ? 1 : 0);
  }
  u8(at: number, v: number): void {
    if (this.fits(at, 1)) this.view.setUint8(at, v);
  }
  i8(at: number, v: number): void {
    if (this.fits(at, 1)) this.view.setInt8(at, v);
  }
  u16(at: number, v: number): void {
    if (this.fits(at, 2)) this.view.setUint16(at, v, true);
  }
  i16(at: number, v: number): void {
    if (this.fits(at, 2)) this.view.setInt16(at, v, true);
  }
  u32(at: number, v: number): void {
    if (this.fits(at, 4)) this.view.setUint32(at, v, true);
  }
  i32(at: number, v: number): void {
    if (this.fits(at, 4)) this.view.setInt32(at, v, true);
  }
  u64(at: number, v: bigint): void {
    if (this.fits(at, 8)) this.view.setBigUint64(at, v, true);
  }
  i64(at: number, v: bigint): void {
    if (this.fits(at, 8)) this.view.setBigInt64(at, v, true);
  }
  f32(at: number, v: number): void {
    if (this.fits(at, 4)) this.view.setFloat32(at, v, true);
  }
  f64(at: number, v: number): void {
    if (this.fits(at, 8)) this.view.setFloat64(at, v, true);
  }

  /** Takes `len` elements of `sz` bytes at `align` past what is written,
   * zeroed, and puts their offset and count at `at`. It answers where they
   * begin, or null where they run past the bytes it is given. */
  list(at: number, len: number, sz: number, align: number): number | null {
    const base = align_to(this.end, align);
    if (!this.fits(base, len * sz)) return null;
    this.buf.fill(0, this.end, base + len * sz);
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
    if (base !== null) this.buf.set(to_bytes(a), base);
  }

  str(at: number, s: string): void {
    const b = te.encode(s);
    const base = this.list(at, b.length, 1, 1);
    if (base !== null) this.buf.set(b, base);
  }

  /** The packet's length, or -1 where it is past the bytes it is given. */
  finish(): number {
    return this.overflow ? -1 : this.end;
  }
}

/** Reads a packet's values where its type says they lie, refusing EBADMSG
 * what lies past the packet, a misaligned list, or a string not UTF-8. */
export class Lift {
  private readonly pkt: U8Array;
  private readonly view: DataView;

  constructor(pkt: U8Array) {
    this.pkt = pkt;
    this.view = new DataView(pkt.buffer, pkt.byteOffset, pkt.byteLength);
  }

  private within(at: number, n: number): number {
    if (at + n > this.pkt.length) throw new XputeError(ERRNO_EBADMSG);
    return at;
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
    if (base % align !== 0 || base + len * sz > this.pkt.length) {
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
    return new ctor(this.pkt.slice(base, base + len * n).buffer);
  }

  private bytes(at: number): U8Array {
    const [base, len] = this.list(at, 1, 1);
    return this.pkt.subarray(base, base + len);
  }

  str(at: number): string {
    try {
      return UTF8.decode(this.bytes(at));
    } catch {
      throw new XputeError(ERRNO_EBADMSG);
    }
  }
}
