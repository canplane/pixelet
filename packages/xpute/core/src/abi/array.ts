// @xpute/core/abi/array.ts

/**
 * AB-backed typed array helpers.
 * Number and BigInt typed arrays are supported in separate sections.
 * The two sections are intentionally not unified due to JS typed array API constraints.
 * Inputs must satisfy the function contracts; contract violations may throw.
 */

import type { f32, f64, i16, i32, i64, i8, u16, u32, u64, u8 } from "./word.ts";

// ============ Number ============

// ---- Canonical aliases ----

export type U8Array<B extends ArrayBufferLike = ArrayBuffer> = Uint8Array<B>;
export type I8Array<B extends ArrayBufferLike = ArrayBuffer> = Int8Array<B>;
export type U16Array<B extends ArrayBufferLike = ArrayBuffer> = Uint16Array<B>;
export type I16Array<B extends ArrayBufferLike = ArrayBuffer> = Int16Array<B>;
export type U32Array<B extends ArrayBufferLike = ArrayBuffer> = Uint32Array<B>;
export type I32Array<B extends ArrayBufferLike = ArrayBuffer> = Int32Array<B>;
export type F32Array<B extends ArrayBufferLike = ArrayBuffer> = Float32Array<B>;
export type F64Array<B extends ArrayBufferLike = ArrayBuffer> = Float64Array<B>;

// ---- Typed array meta ----

export type TypedArray<B extends ArrayBufferLike = ArrayBuffer> =
  | U8Array<B>
  | I8Array<B>
  | U16Array<B>
  | I16Array<B>
  | U32Array<B>
  | I32Array<B>
  | F32Array<B>
  | F64Array<B>;

export type ElementOf<T extends TypedArray> = T extends U8Array ? u8
  : T extends I8Array ? i8
  : T extends U16Array ? u16
  : T extends I16Array ? i16
  : T extends U32Array ? u32
  : T extends I32Array ? i32
  : T extends F32Array ? f32
  : T extends F64Array ? f64
  : never;

export type TypedArrayCtor<T extends TypedArray = TypedArray> = {
  new (length: number): T;
  new (elements: ArrayLike<number>): T;
  new (buffer: ArrayBufferLike, byteOffset?: number, length?: number): T;
  readonly BYTES_PER_ELEMENT: number;
};

// ---- View normalization ----

/** True if vals is an ArrayBuffer. */
export const is_ab = (vals: unknown): vals is ArrayBuffer => vals instanceof ArrayBuffer;

/** True if vals is a SharedArrayBuffer. */
export const is_sab = (vals: unknown): vals is SharedArrayBuffer => typeof SharedArrayBuffer !== "undefined" && vals instanceof SharedArrayBuffer;

/** True if vals is one of the supported Number typed arrays. */
export const is_ta = (vals: unknown): vals is TypedArray =>
  vals instanceof Uint8Array ||
  vals instanceof Int8Array ||
  vals instanceof Uint16Array ||
  vals instanceof Int16Array ||
  vals instanceof Uint32Array ||
  vals instanceof Int32Array ||
  vals instanceof Float32Array ||
  vals instanceof Float64Array;

/** True if vals is a Number typed array backed by ArrayBuffer. */
export const is_ta_ab = <T extends TypedArray>(
  vals: T | null | undefined,
): vals is T & { buffer: ArrayBuffer } => !!vals && is_ta(vals) && is_ab(vals.buffer);

/** True if vals is a Number typed array backed by SharedArrayBuffer. */
export const is_ta_sab = <T extends TypedArray>(
  vals: T | null | undefined,
): vals is T & { buffer: SharedArrayBuffer } => !!vals && is_ta(vals) && is_sab(vals.buffer);

/**
 * Normalize common boundary inputs into an AB-backed T.
 * Accepts ArrayBuffer, Number typed array views, and array-like values.
 * Invalid inputs are treated as contract violations and may throw.
 * - exact T over AB: return as-is
 * - ArrayBuffer: zero-copy wrap
 * - other Number typed array view over AB: zero-copy rewrap
 * - Number typed array view over SAB: copy
 * - plain ArrayLike: copy
 */
export const as_ta_ab = <T extends TypedArray>(
  vals: ArrayBuffer | TypedArray | ArrayLike<number>,
  ctor: TypedArrayCtor<T>,
): T => {
  // exact type over AB
  if (vals instanceof ctor && is_ab(vals.buffer)) return vals;

  if (is_ab(vals)) return new ctor(vals);

  // typed array view
  if (is_ta(vals)) {
    const buf = vals.buffer;

    // AB-backed: preserve current window zero-copy
    if (is_ab(buf)) return new ctor(buf, vals.byteOffset, vals.length);

    // SAB-backed: copy into fresh AB-backed storage
    if (is_sab(buf)) {
      const out = new ctor(vals.length);
      out.set(vals);
      return out;
    }
  }

  // plain ArrayLike
  return new ctor(vals);
};

/**
 * Ensure vals is an instance of T.
 * - exact T: return as-is (zero-copy)
 * - plain ArrayLike: copy into fresh AB-backed T
 */
export const to_ta = <T extends TypedArray>(arr: T | number[], ctor: TypedArrayCtor<T>): T => {
  return (arr instanceof ctor) ? (arr as T) : new ctor(arr);
};

/**
 * Copy a typed array window into fresh AB-backed T.
 * This is the forced-copy counterpart to as_ta_ab().
 */
export const copy_ab_view = <T extends TypedArray>(
  buf: ArrayBuffer,
  off: u32,
  len: u32,
  ctor: TypedArrayCtor<T>,
): T => {
  const out: T = new ctor(len);
  out.set(new ctor(buf, off, len));
  return out;
};

// ---- Bytes (U8Array) ----

/** Canonical byte carrier view used across the codebase. */
export type Bytes = U8Array;

/** True if vals is canonical AB-backed bytes. */
export const is_bytes = (vals: unknown): vals is Bytes => vals instanceof Uint8Array && is_ab(vals.buffer);

/** Thin byte-specialized wrapper over as_ta_ab(Uint8Array). */
export const as_bytes = (vals: ArrayBuffer | TypedArray | ArrayLike<number>): Bytes => as_ta_ab(vals, Uint8Array);

/**
 * Zero-copy cast of any typed array view into a byte view (U8Array).
 * Preserves the source view window (byteOffset/byteLength).
 */
export const to_bytes = (arr: TypedArray | BigTypedArray): Bytes => new Uint8Array(arr.buffer, arr.byteOffset, arr.byteLength);

/** Copy a byte window into fresh AB-backed bytes. */
export const copy_bytes = (buf: ArrayBuffer, off: u32, nbyte: u32): Bytes => copy_ab_view(buf, off, nbyte, Uint8Array);

// ============ BigInt ============

// ---- Canonical aliases ----

export type U64Array<B extends ArrayBufferLike = ArrayBuffer> = BigUint64Array<B>;
export type I64Array<B extends ArrayBufferLike = ArrayBuffer> = BigInt64Array<B>;

// ---- Typed array meta ----

export type BigTypedArray<B extends ArrayBufferLike = ArrayBuffer> =
  | U64Array<B>
  | I64Array<B>;

export type BigElementOf<T extends BigTypedArray> = T extends U64Array ? u64
  : T extends I64Array ? i64
  : never;

export type BigTypedArrayCtor<T extends BigTypedArray = BigTypedArray> = {
  new (length: number): T;
  new (elements: ArrayLike<bigint>): T;
  new (buffer: ArrayBufferLike, byteOffset?: number, length?: number): T;
  readonly BYTES_PER_ELEMENT: number;
};

// ---- View normalization ----

/** True if vals is one of the supported BigInt typed arrays. */
export const is_bta = (vals: unknown): vals is BigTypedArray =>
  vals instanceof BigUint64Array ||
  vals instanceof BigInt64Array;

/** True if vals is a BigInt typed array backed by ArrayBuffer. */
export const is_bta_ab = <T extends BigTypedArray>(
  vals: T | null | undefined,
): vals is T & { buffer: ArrayBuffer } => !!vals && is_bta(vals) && is_ab(vals.buffer);

/** True if vals is a BigInt typed array backed by SharedArrayBuffer. */
export const is_bta_sab = <T extends BigTypedArray>(
  vals: T | null | undefined,
): vals is T & { buffer: SharedArrayBuffer } => !!vals && is_bta(vals) && is_sab(vals.buffer);

/**
 * Normalize common boundary inputs into an AB-backed BigInt T.
 * Accepts ArrayBuffer, BigInt typed array views, and array-like values.
 * Invalid inputs are treated as contract violations and may throw.
 * - exact T over AB: return as-is
 * - ArrayBuffer: zero-copy wrap
 * - other BigInt typed array view over AB: zero-copy rewrap
 * - BigInt typed array view over SAB: copy
 * - plain ArrayLike: copy
 */
export const as_bta_ab = <T extends BigTypedArray>(
  vals: ArrayBuffer | BigTypedArray | ArrayLike<bigint>,
  ctor: BigTypedArrayCtor<T>,
): T => {
  // exact type over AB
  if (vals instanceof ctor && is_ab(vals.buffer)) return vals;

  if (is_ab(vals)) return new ctor(vals);

  if (is_bta(vals)) {
    const buf = vals.buffer;

    // AB-backed: preserve current window zero-copy
    if (is_ab(buf)) return new ctor(buf, vals.byteOffset, vals.length);

    // SAB-backed: copy into fresh AB-backed storage
    if (is_sab(buf)) {
      const out = new ctor(vals.length);
      out.set(vals);
      return out;
    }
  }

  // plain ArrayLike
  return new ctor(vals);
};

/**
 * Ensure vals is an instance of BigInt T.
 * - exact T: return as-is (zero-copy)
 * - plain ArrayLike: copy into fresh AB-backed T
 */
export const to_bta = <T extends BigTypedArray>(arr: T | bigint[], ctor: BigTypedArrayCtor<T>): T => {
  return (arr instanceof ctor) ? (arr as T) : new ctor(arr);
};

/**
 * Copy a BigInt typed array window into fresh AB-backed T.
 * This is the forced-copy counterpart to as_bta_ab().
 */
export const copy_bta_view = <T extends BigTypedArray>(
  buf: ArrayBuffer,
  off: u32,
  len: u32,
  ctor: BigTypedArrayCtor<T>,
): T => {
  const out: T = new ctor(len);
  out.set(new ctor(buf, off, len));
  return out;
};

// ============ Any ============

/** Minimal typed-array-like view shape used by generic packet readers. */
export type AnyTypedArray<B extends ArrayBufferLike = ArrayBuffer> =
  | U8Array<B>
  | I8Array<B>
  | U16Array<B>
  | I16Array<B>
  | U32Array<B>
  | I32Array<B>
  | F32Array<B>
  | F64Array<B>
  | U64Array<B>
  | I64Array<B>;

export type AnyElementOf<T extends AnyTypedArray> = T extends U8Array ? u8
  : T extends I8Array ? i8
  : T extends U16Array ? u16
  : T extends I16Array ? i16
  : T extends U32Array ? u32
  : T extends I32Array ? i32
  : T extends F32Array ? f32
  : T extends F64Array ? f64
  : T extends U64Array ? u64
  : T extends I64Array ? i64
  : never;

/** Minimal ctor shape for zero-copy typed-array view rewraps. */
export interface AnyTypedArrayCtor<T extends AnyTypedArray> {
  new (length: number): T;
  new (buffer: ArrayBufferLike, byteOffset: number, length: number): T;
  readonly BYTES_PER_ELEMENT: number;
}
