// @xpute/spec/io/marshal/struct.ts

/**
 * @deprecated
 * Legacy schema-known fixed-layout wire format.
 *
 * This module is retained for reference, legacy data, and migration purposes.
 * New structured payload work should prefer ast.ts as the canonical hierarchical model,
 * or tlv.ts for sequential self-delimiting streams.
 *
 * This module may later be replaced by an ast-backed schema/view layer.
 */

/**
 * Struct: schema-known, non-self-describing, C-style fixed layout with offset refs.
 *
 * Scope
 * -----
 * - This format is a schema-known fixed-layout ABI tool, not the primary general-purpose marshal format.
 * - This module encodes and decodes a schema-fixed binary object tree.
 * - Field order, field kind, offsets, and nesting are owned entirely by the caller-provided schema.
 * - The wire bytes are NOT self-describing beyond the fixed outer header.
 *
 * Wire Layout
 * -----------
 * [ header | inline body | tail... ]
 *
 * header (16 bytes, little-endian):
 * - u32 magic
 * - u16 version
 * - u16 flags
 * - u32 nbyte
 * - u32 reserved
 *
 * inline body:
 * - Fixed-size region defined by schema.size.
 * - Scalar fields are stored inline at fixed offsets.
 * - Ref fields store either:
 *   - [u32 off][u32 len]  for str / array
 *   - [u32 off]           for nested struct
 *
 * tail:
 * - Variable-size appended payload area for strings, arrays, and nested structs.
 * - Each payload is aligned to its natural alignment, with a minimum alignment of 4 bytes.
 * - Ref offsets are absolute offsets from the beginning of the current struct blob.
 *
 * Ref Semantics
 * -------------
 * - str:        [off,len], len is byte length of UTF-8 payload
 * - array:      [off,len], len is element count, not byte size
 * - struct:     [off], points to another full struct blob beginning with its own header
 *
 * Nested Struct Model
 * -------------------
 * - Nested structs are serialized as full child blobs in the tail.
 * - Each child carries its own header and size domain.
 * - Parent struct refs point to the child's blob start.
 *
 * Validation / Trust Boundary
 * ---------------------------
 * - read_header(): raw header read only; no semantic validation
 * - assert_header(): validates header invariants for a complete struct blob
 * - StructReader assumes the provided schema matches the actual body layout.
 * - Schema mismatch is a caller contract violation and may yield EBADMSG/EINVAL or invalid interpretation.
 *
 * Alignment / Layout Rules
 * ------------------------
 * - Scalars <=32-bit occupy 4-byte slots.
 * - 64-bit scalars occupy 8-byte slots.
 * - str/array refs occupy 8-byte slots ([off,len]).
 * - nested struct refs occupy 4-byte slots ([off]).
 * - Schema size is padded to the maximum alignment used by the schema.
 * - Tail payload alignment is computed relative to the current blob base.
 *
 * Invariants
 * ----------
 * - Empty schemas are rejected.
 * - Duplicate field names are rejected.
 * - Header size is fixed at 16 bytes.
 * - flags and reserved must currently be zero.
 * - nbyte must exactly match the provided blob length.
 * - Writer is append-only until finish(); write-after-finish is invalid.
 *
 * Non-Goals
 * ---------
 * - No schema evolution protocol beyond the outer version field.
 * - No self-description of field names, field counts, or field kinds on wire.
 * - No checksum/hash/compression/encryption inside this format.
 * - No pointer rebasing; refs are absolute within the current blob.
 */

import type { f32, f64, i16, i32, i64, i8, u16, u32, u64, u8 } from "@xpute/spec/lib/kit/word.ts";
import type { Bytes, F32Array, F64Array, I16Array, I32Array, I64Array, I8Array, U16Array, U32Array, U64Array, U8Array } from "@xpute/spec/lib/kit/array.ts";
import * as encoding from "@xpute/spec/lib/kit/encoding.ts";
import type { Stream } from "@xpute/spec/io/scalar.ts";
import * as scalar from "@xpute/spec/io/scalar.ts";
import { Errno } from "@xpute/spec/sys/errno.ts";
import { MarshalError } from "@xpute/spec/sys/error.ts";

// ============ ABI ============

const STRUCT_HEADER_SZ: u32 = 16;

export const STRUCT_MAGIC: u32 = 0x54525543; // "TRUC" or change later
export const STRUCT_VERSION: u16 = 1;

// ============ Header ============

// ---- Header types ----

export type StructHeader = {
  magic: u32;
  version: u16;
  flags: u16;
  nbyte: u32;
  reserved: u32;
};

// ---- Header IO ----

function put_header(s: Stream, h: StructHeader): void {
  scalar.putu32(s, h.magic);
  scalar.putu16(s, h.version);
  scalar.putu16(s, h.flags);
  scalar.putu32(s, h.nbyte);
  scalar.putu32(s, h.reserved);
}

// raw header read only; structural validation is performed by assert_header()
export function read_header(buf: Bytes): StructHeader {
  if (buf.byteLength < STRUCT_HEADER_SZ) {
    throw new MarshalError(Errno.EBADMSG, `struct: truncated header`);
  }

  const s = scalar.stream(buf);
  return {
    magic: scalar.getu32(s),
    version: scalar.getu16(s),
    flags: scalar.getu16(s),
    nbyte: scalar.getu32(s),
    reserved: scalar.getu32(s),
  };
}

// ---- Header checks ----

// validates outer blob header invariants only; does not validate schema/body semantics
export function assert_header(buf: Bytes): StructHeader {
  const h = read_header(buf);

  if (h.magic !== STRUCT_MAGIC) {
    throw new MarshalError(Errno.EBADMSG, "struct: bad magic");
  }
  if (h.version !== STRUCT_VERSION) {
    throw new MarshalError(Errno.EBADMSG, `struct: bad version ${h.version}`);
  }
  if (h.flags !== 0) {
    throw new MarshalError(Errno.EBADMSG, `struct: bad flags ${h.flags}`);
  }
  if (h.nbyte !== buf.byteLength) {
    throw new MarshalError(Errno.EBADMSG, `struct: bad nbyte ${h.nbyte}`);
  }
  if (h.reserved !== 0) {
    throw new MarshalError(Errno.EBADMSG, `struct: bad reserved ${h.reserved}`);
  }

  return h;
}

// ============ Elements ============

// ---- Elements ----

// Field kind domain for schema-defined struct layout (not a stream control domain).
export const enum ElementKind {
  BOOL = 0x01,

  U8 = 0x02,
  I8 = 0x03,
  U16 = 0x04,
  I16 = 0x05,
  U32 = 0x06,
  I32 = 0x07,
  U64 = 0x08,
  I64 = 0x09,
  F32 = 0x0a,
  F64 = 0x0b,

  STR = 0x10,

  U8_ARRAY = 0x20, // also used for opaque binary payloads in schema-known layouts
  I8_ARRAY = 0x21,
  U16_ARRAY = 0x22,
  I16_ARRAY = 0x23,
  U32_ARRAY = 0x24,
  I32_ARRAY = 0x25,
  U64_ARRAY = 0x26,
  I64_ARRAY = 0x27,
  F32_ARRAY = 0x28,
  F64_ARRAY = 0x29,

  STRUCT = 0x40,
}

export type ScalarKind =
  | ElementKind.BOOL
  | ElementKind.U8
  | ElementKind.I8
  | ElementKind.U16
  | ElementKind.I16
  | ElementKind.U32
  | ElementKind.I32
  | ElementKind.U64
  | ElementKind.I64
  | ElementKind.F32
  | ElementKind.F64;

export type ArrayKind =
  | ElementKind.U8_ARRAY
  | ElementKind.I8_ARRAY
  | ElementKind.U16_ARRAY
  | ElementKind.I16_ARRAY
  | ElementKind.U32_ARRAY
  | ElementKind.I32_ARRAY
  | ElementKind.U64_ARRAY
  | ElementKind.I64_ARRAY
  | ElementKind.F32_ARRAY
  | ElementKind.F64_ARRAY;

// ---- Schema types ----

// kind describes the field payload contract at schema level, not wire self-description
export type StructField =
  | { name: string; kind: ElementKind.BOOL; off: u32 }
  | { name: string; kind: ElementKind.U8; off: u32 }
  | { name: string; kind: ElementKind.I8; off: u32 }
  | { name: string; kind: ElementKind.U16; off: u32 }
  | { name: string; kind: ElementKind.I16; off: u32 }
  | { name: string; kind: ElementKind.U32; off: u32 }
  | { name: string; kind: ElementKind.I32; off: u32 }
  | { name: string; kind: ElementKind.U64; off: u32 }
  | { name: string; kind: ElementKind.I64; off: u32 }
  | { name: string; kind: ElementKind.F32; off: u32 }
  | { name: string; kind: ElementKind.F64; off: u32 }
  | { name: string; kind: ElementKind.STR; off: u32 } // [u32 off][u32 len]
  | { name: string; kind: ElementKind.U8_ARRAY; off: u32 }
  | { name: string; kind: ElementKind.I8_ARRAY; off: u32 }
  | { name: string; kind: ElementKind.U16_ARRAY; off: u32 }
  | { name: string; kind: ElementKind.I16_ARRAY; off: u32 }
  | { name: string; kind: ElementKind.U32_ARRAY; off: u32 }
  | { name: string; kind: ElementKind.I32_ARRAY; off: u32 }
  | { name: string; kind: ElementKind.U64_ARRAY; off: u32 }
  | { name: string; kind: ElementKind.I64_ARRAY; off: u32 }
  | { name: string; kind: ElementKind.F32_ARRAY; off: u32 }
  | { name: string; kind: ElementKind.F64_ARRAY; off: u32 }
  | { name: string; kind: ElementKind.STRUCT; off: u32; schema: StructSchema }; // [u32 off]

export type StructSchema = {
  name?: string;
  // canonical field order = declaration order
  // wire layout is derived from this order + per-field alignment.
  // Fixed inline body size of the struct body region, already padded/aligned by schema.
  size: u32;
  fields: readonly StructField[];

  // internal lookup cache only; do not use for canonical field order
  _lut?: ReadonlyMap<string, StructField>;
};

// ============ Schema builder ============

// ---- Builder input ----

type StructFieldInit =
  | { name: string; kind: ElementKind.BOOL }
  | { name: string; kind: ElementKind.U8 }
  | { name: string; kind: ElementKind.I8 }
  | { name: string; kind: ElementKind.U16 }
  | { name: string; kind: ElementKind.I16 }
  | { name: string; kind: ElementKind.U32 }
  | { name: string; kind: ElementKind.I32 }
  | { name: string; kind: ElementKind.U64 }
  | { name: string; kind: ElementKind.I64 }
  | { name: string; kind: ElementKind.F32 }
  | { name: string; kind: ElementKind.F64 }
  | { name: string; kind: ElementKind.STR }
  | { name: string; kind: ElementKind.U8_ARRAY }
  | { name: string; kind: ElementKind.I8_ARRAY }
  | { name: string; kind: ElementKind.U16_ARRAY }
  | { name: string; kind: ElementKind.I16_ARRAY }
  | { name: string; kind: ElementKind.U32_ARRAY }
  | { name: string; kind: ElementKind.I32_ARRAY }
  | { name: string; kind: ElementKind.U64_ARRAY }
  | { name: string; kind: ElementKind.I64_ARRAY }
  | { name: string; kind: ElementKind.F32_ARRAY }
  | { name: string; kind: ElementKind.F64_ARRAY }
  | { name: string; kind: ElementKind.STRUCT; schema: StructSchema };

// ---- Builder layout helpers ----

const align_of_kind = (kind: ElementKind): u32 => {
  switch (kind) {
    case ElementKind.U64:
    case ElementKind.I64:
    case ElementKind.F64:
    case ElementKind.U64_ARRAY:
    case ElementKind.I64_ARRAY:
    case ElementKind.F64_ARRAY:
      return 8;
    default:
      return 4;
  }
};

const slot_size_of_kind = (kind: ElementKind): u32 => {
  switch (kind) {
    case ElementKind.U64:
    case ElementKind.I64:
    case ElementKind.F64:
      return 8;
    case ElementKind.STR:
    case ElementKind.U8_ARRAY:
    case ElementKind.I8_ARRAY:
    case ElementKind.U16_ARRAY:
    case ElementKind.I16_ARRAY:
    case ElementKind.U32_ARRAY:
    case ElementKind.I32_ARRAY:
    case ElementKind.U64_ARRAY:
    case ElementKind.I64_ARRAY:
    case ElementKind.F32_ARRAY:
    case ElementKind.F64_ARRAY:
      return 8; // [off,len]
    case ElementKind.STRUCT:
      return 4; // [off]
    default:
      return 4;
  }
};

function schema_align_of(schema: StructSchema): u32 {
  let max_align: u32 = 4;

  for (let i = 0; i < schema.fields.length; i++) {
    const f = schema.fields[i];
    let a: u32;

    if (f.kind === ElementKind.STRUCT) a = schema_align_of(f.schema);
    else a = align_of_kind(f.kind);

    if (a > max_align) max_align = a;
  }

  return max_align;
}

const align_of_field = (f: StructFieldInit): u32 => f.kind === ElementKind.STRUCT ? schema_align_of(f.schema) : align_of_kind(f.kind);

const slot_size_of_field = (f: StructFieldInit): u32 => slot_size_of_kind(f.kind);

// ---- Builder ----

export class StructBuilder {
  private _fields: StructFieldInit[] = [];

  constructor(private readonly _name?: string) {}

  bool(name: string): this {
    this._fields.push({ name, kind: ElementKind.BOOL });
    return this;
  }
  u8(name: string): this {
    this._fields.push({ name, kind: ElementKind.U8 });
    return this;
  }
  i8(name: string): this {
    this._fields.push({ name, kind: ElementKind.I8 });
    return this;
  }
  u16(name: string): this {
    this._fields.push({ name, kind: ElementKind.U16 });
    return this;
  }
  i16(name: string): this {
    this._fields.push({ name, kind: ElementKind.I16 });
    return this;
  }
  u32(name: string): this {
    this._fields.push({ name, kind: ElementKind.U32 });
    return this;
  }
  i32(name: string): this {
    this._fields.push({ name, kind: ElementKind.I32 });
    return this;
  }
  u64(name: string): this {
    this._fields.push({ name, kind: ElementKind.U64 });
    return this;
  }
  i64(name: string): this {
    this._fields.push({ name, kind: ElementKind.I64 });
    return this;
  }
  f32(name: string): this {
    this._fields.push({ name, kind: ElementKind.F32 });
    return this;
  }
  f64(name: string): this {
    this._fields.push({ name, kind: ElementKind.F64 });
    return this;
  }

  str(name: string): this {
    this._fields.push({ name, kind: ElementKind.STR });
    return this;
  }

  u8_array(name: string): this {
    this._fields.push({ name, kind: ElementKind.U8_ARRAY });
    return this;
  }
  i8_array(name: string): this {
    this._fields.push({ name, kind: ElementKind.I8_ARRAY });
    return this;
  }
  u16_array(name: string): this {
    this._fields.push({ name, kind: ElementKind.U16_ARRAY });
    return this;
  }
  i16_array(name: string): this {
    this._fields.push({ name, kind: ElementKind.I16_ARRAY });
    return this;
  }
  u32_array(name: string): this {
    this._fields.push({ name, kind: ElementKind.U32_ARRAY });
    return this;
  }
  i32_array(name: string): this {
    this._fields.push({ name, kind: ElementKind.I32_ARRAY });
    return this;
  }
  u64_array(name: string): this {
    this._fields.push({ name, kind: ElementKind.U64_ARRAY });
    return this;
  }
  i64_array(name: string): this {
    this._fields.push({ name, kind: ElementKind.I64_ARRAY });
    return this;
  }
  f32_array(name: string): this {
    this._fields.push({ name, kind: ElementKind.F32_ARRAY });
    return this;
  }
  f64_array(name: string): this {
    this._fields.push({ name, kind: ElementKind.F64_ARRAY });
    return this;
  }

  struct(name: string, schema: StructSchema): this {
    this._fields.push({ name, kind: ElementKind.STRUCT, schema });
    return this;
  }

  // canonical field order = declaration order
  finish(): StructSchema {
    if (this._fields.length === 0) throw new MarshalError(Errno.EINVAL, "struct: empty schema");
    const seen = new Set<string>();

    const fields: StructField[] = [];
    const _lut = new Map<string, StructField>(); // lookup cache; order/layout remains fields[] declaration order
    let off: u32 = 0;
    let max_align: u32 = 4;

    for (let i = 0; i < this._fields.length; i++) {
      const f = this._fields[i];

      if (seen.has(f.name)) throw new MarshalError(Errno.EEXIST, `struct: duplicate field '${f.name}'`);
      seen.add(f.name);

      const a = align_of_field(f);
      const sz = slot_size_of_field(f);
      off = ALIGN(off, a);
      if (a > max_align) max_align = a;

      let field: StructField;
      if (f.kind === ElementKind.STRUCT) field = { name: f.name, kind: f.kind, off, schema: f.schema };
      else field = { name: f.name, kind: f.kind, off };

      fields.push(field);
      _lut.set(field.name, field);

      off += sz;
    }

    return {
      name: this._name,
      size: ALIGN(off, max_align),
      fields,

      _lut,
    };
  }
}

const AT = (off: u32): u32 => STRUCT_HEADER_SZ + off;

// ============ Shared helpers ============

// ---- Schema lookup ----

function field_of(schema: StructSchema, name: string): StructField {
  const _lut = schema._lut;
  if (_lut) {
    const f = _lut.get(name);
    if (f) return f;
    throw new MarshalError(Errno.ENOENT, `struct: unknown field '${name}'`);
  }

  for (let i = 0; i < schema.fields.length; i++) {
    const f = schema.fields[i];
    if (f.name === name) return f;
  }
  throw new MarshalError(Errno.ENOENT, `struct: unknown field '${name}'`);
}

// ---- Layout/alignment ----

const ALIGN = (n: u32, a: u32): u32 => (n + (a - 1)) & ~(a - 1);

const ALIGN_TAIL_OFF = (base: u32, tail_off: u32, a: u32): u32 => ALIGN(base + tail_off, a) - base;

// ---- View helpers ----

const VIEW_BYTES = (x: { buffer: ArrayBuffer; byteOffset: u32; byteLength: u32 }): Bytes => new Uint8Array(x.buffer, x.byteOffset, x.byteLength);

// ============ Writer ============

// ---- Writer core ----

export class StructWriter {
  private _body: Bytes;
  private _tail: Bytes;
  private _body_len: u32;
  private _tail_len: u32;
  private _sealed = false;

  constructor(
    private readonly _schema: StructSchema,
    body_cap: u32 = 256,
    tail_cap: u32 = 1024,
  ) {
    this._body = new Uint8Array(Math.max(body_cap, _schema.size));
    this._tail = new Uint8Array(tail_cap);
    this._body_len = _schema.size;
    this._tail_len = 0;
  }

  private _check_unsealed(): void {
    if (this._sealed) throw new MarshalError(Errno.EBADMSG, "struct: write after finish()");
  }

  private _ensure_tail(nbyte: u32): void {
    const need = this._tail_len + nbyte;
    if (need <= this._tail.byteLength) return;

    const cap: u32 = Math.max(this._tail.byteLength << 1, need);
    const next: Bytes = new Uint8Array(cap);
    next.set(this._tail.subarray(0, this._tail_len));
    this._tail = next;
  }

  private _tail_alloc(nbyte: u32, a: u32 = 4): u32 {
    this._check_unsealed();

    const base = STRUCT_HEADER_SZ + this._body_len;
    const off = ALIGN_TAIL_OFF(base, this._tail_len, a);
    const pad = off - this._tail_len;
    if (pad) {
      this._ensure_tail(pad);
      this._tail.fill(0, this._tail_len, off);
      this._tail_len = off;
    }

    this._ensure_tail(nbyte);
    const out = this._tail_len;
    this._tail_len += nbyte;
    return out;
  }

  private _body_stream(off: u32): Stream {
    return scalar.stream(this._body, off);
  }

  private _put_ref(off: u32, ref_off: u32, len: u32): void {
    const s = this._body_stream(off);
    scalar.putu32(s, ref_off);
    scalar.putu32(s, len);
  }

  private _put_struct_ref(off: u32, ref_off: u32): void {
    const s = this._body_stream(off);
    scalar.putu32(s, ref_off);
  }

  // ---- Scalar writers ----

  bool(name: string, x: boolean): this {
    this._check_unsealed();
    const f = field_of(this._schema, name);
    if (f.kind !== ElementKind.BOOL) throw new MarshalError(Errno.EINVAL, `struct: field '${name}' is not bool`);
    const s = this._body_stream(f.off);
    scalar.putu32(s, x ? 1 : 0);
    return this;
  }

  u8(name: string, x: u8): this {
    this._check_unsealed();
    const f = field_of(this._schema, name);
    if (f.kind !== ElementKind.U8) throw new MarshalError(Errno.EINVAL, `struct: field '${name}' is not u8`);
    const s = this._body_stream(f.off);
    scalar.putu32(s, x);
    return this;
  }
  i8(name: string, x: i8): this {
    this._check_unsealed();
    const f = field_of(this._schema, name);
    if (f.kind !== ElementKind.I8) throw new MarshalError(Errno.EINVAL, `struct: field '${name}' is not i8`);
    const s = this._body_stream(f.off);
    scalar.puti32(s, x);
    return this;
  }
  u16(name: string, x: u16): this {
    this._check_unsealed();
    const f = field_of(this._schema, name);
    if (f.kind !== ElementKind.U16) throw new MarshalError(Errno.EINVAL, `struct: field '${name}' is not u16`);
    const s = this._body_stream(f.off);
    scalar.putu32(s, x);
    return this;
  }
  i16(name: string, x: i16): this {
    this._check_unsealed();
    const f = field_of(this._schema, name);
    if (f.kind !== ElementKind.I16) throw new MarshalError(Errno.EINVAL, `struct: field '${name}' is not i16`);
    const s = this._body_stream(f.off);
    scalar.puti32(s, x);
    return this;
  }
  u32(name: string, x: u32): this {
    this._check_unsealed();
    const f = field_of(this._schema, name);
    if (f.kind !== ElementKind.U32) throw new MarshalError(Errno.EINVAL, `struct: field '${name}' is not u32`);
    const s = this._body_stream(f.off);
    scalar.putu32(s, x);
    return this;
  }
  i32(name: string, x: i32): this {
    this._check_unsealed();
    const f = field_of(this._schema, name);
    if (f.kind !== ElementKind.I32) throw new MarshalError(Errno.EINVAL, `struct: field '${name}' is not i32`);
    const s = this._body_stream(f.off);
    scalar.puti32(s, x);
    return this;
  }
  u64(name: string, x: u64): this {
    this._check_unsealed();
    const f = field_of(this._schema, name);
    if (f.kind !== ElementKind.U64) throw new MarshalError(Errno.EINVAL, `struct: field '${name}' is not u64`);
    const s = this._body_stream(f.off);
    scalar.putu64(s, x);
    return this;
  }
  i64(name: string, x: i64): this {
    this._check_unsealed();
    const f = field_of(this._schema, name);
    if (f.kind !== ElementKind.I64) throw new MarshalError(Errno.EINVAL, `struct: field '${name}' is not i64`);
    const s = this._body_stream(f.off);
    scalar.puti64(s, x);
    return this;
  }
  f32(name: string, x: f32): this {
    this._check_unsealed();
    const f = field_of(this._schema, name);
    if (f.kind !== ElementKind.F32) throw new MarshalError(Errno.EINVAL, `struct: field '${name}' is not f32`);
    const s = this._body_stream(f.off);
    scalar.putf32(s, x);
    return this;
  }
  f64(name: string, x: f64): this {
    this._check_unsealed();
    const f = field_of(this._schema, name);
    if (f.kind !== ElementKind.F64) throw new MarshalError(Errno.EINVAL, `struct: field '${name}' is not f64`);
    const s = this._body_stream(f.off);
    scalar.putf64(s, x);
    return this;
  }

  // ---- Ref writers ----

  str(name: string, x: string): this {
    this._check_unsealed();
    const f = field_of(this._schema, name);
    if (f.kind !== ElementKind.STR) throw new MarshalError(Errno.EINVAL, `struct: field '${name}' is not str`);
    const bytes = encoding.te.encode(x);
    const tail_off = this._tail_alloc(bytes.byteLength, align_of_kind(f.kind));
    this._tail.set(bytes, tail_off);
    this._put_ref(f.off, STRUCT_HEADER_SZ + this._body_len + tail_off, bytes.byteLength);
    return this;
  }

  private _put_array(name: string, kind: ArrayKind, buf: Bytes, elem_n: u32): this {
    this._check_unsealed();
    const f = field_of(this._schema, name);
    if (f.kind !== kind) throw new MarshalError(Errno.EINVAL, `struct: field '${name}' kind mismatch`);
    const tail_off = this._tail_alloc(buf.byteLength, align_of_kind(kind));
    this._tail.set(buf, tail_off);
    this._put_ref(f.off, STRUCT_HEADER_SZ + this._body_len + tail_off, elem_n);
    return this;
  }

  u8_array(name: string, x: U8Array): this {
    return this._put_array(name, ElementKind.U8_ARRAY, VIEW_BYTES(x), x.length);
  }
  i8_array(name: string, x: I8Array): this {
    return this._put_array(name, ElementKind.I8_ARRAY, VIEW_BYTES(x), x.length);
  }
  u16_array(name: string, x: U16Array): this {
    return this._put_array(name, ElementKind.U16_ARRAY, VIEW_BYTES(x), x.length);
  }
  i16_array(name: string, x: I16Array): this {
    return this._put_array(name, ElementKind.I16_ARRAY, VIEW_BYTES(x), x.length);
  }
  u32_array(name: string, x: U32Array): this {
    return this._put_array(name, ElementKind.U32_ARRAY, VIEW_BYTES(x), x.length);
  }
  i32_array(name: string, x: I32Array): this {
    return this._put_array(name, ElementKind.I32_ARRAY, VIEW_BYTES(x), x.length);
  }
  u64_array(name: string, x: U64Array): this {
    return this._put_array(name, ElementKind.U64_ARRAY, VIEW_BYTES(x), x.length);
  }
  i64_array(name: string, x: I64Array): this {
    return this._put_array(name, ElementKind.I64_ARRAY, VIEW_BYTES(x), x.length);
  }
  f32_array(name: string, x: F32Array): this {
    return this._put_array(name, ElementKind.F32_ARRAY, VIEW_BYTES(x), x.length);
  }
  f64_array(name: string, x: F64Array): this {
    return this._put_array(name, ElementKind.F64_ARRAY, VIEW_BYTES(x), x.length);
  }

  struct(name: string, fn: (w: StructWriter) => void): this {
    this._check_unsealed();
    const f = field_of(this._schema, name);
    if (f.kind !== ElementKind.STRUCT) throw new MarshalError(Errno.EINVAL, `struct: field '${name}' is not struct`);

    const child = new StructWriter(f.schema);
    fn(child);
    const bytes = child.finish();
    const tail_off = this._tail_alloc(bytes.byteLength, schema_align_of(f.schema));
    this._tail.set(bytes, tail_off);
    this._put_struct_ref(f.off, STRUCT_HEADER_SZ + this._body_len + tail_off);
    return this;
  }

  data(): Bytes {
    const total = STRUCT_HEADER_SZ + this._body_len + ALIGN(this._tail_len, 4);
    const out: Bytes = new Uint8Array(total);
    const s = scalar.stream(out);

    put_header(s, {
      magic: STRUCT_MAGIC,
      version: STRUCT_VERSION,
      flags: 0,
      nbyte: total,
      reserved: 0,
    });

    out.set(this._body.subarray(0, this._body_len), STRUCT_HEADER_SZ);
    out.set(this._tail.subarray(0, this._tail_len), STRUCT_HEADER_SZ + this._body_len);
    return out;
  }

  finish(): Bytes {
    if (!this._sealed) this._sealed = true;
    return this.data();
  }
}

// ============ Reader ============

// ---- Reader core ----

export class StructReader {
  private readonly _view: DataView;

  constructor(
    private readonly _schema: StructSchema,
    private readonly _buf: Bytes,
  ) {
    this._view = new DataView(_buf.buffer, _buf.byteOffset, _buf.byteLength);

    assert_header(_buf);
  }

  private _field(name: string): StructField {
    return field_of(this._schema, name);
  }

  private _ref_at(off: u32): { off: u32; len: u32 } {
    const s = scalar.stream(this._buf, AT(off));
    return {
      off: scalar.getu32(s),
      len: scalar.getu32(s),
    };
  }

  private _struct_ref_at(off: u32): u32 {
    const s = scalar.stream(this._buf, AT(off));
    return scalar.getu32(s);
  }

  private _array_ref_at(name: string, kind: ArrayKind): { off: u32; len: u32 } {
    const f = this._field(name);
    if (f.kind !== kind) throw new MarshalError(Errno.EINVAL, `struct: field '${name}' kind mismatch`);
    return this._ref_at(f.off);
  }

  // ---- Scalar readers ----

  bool(name: string): boolean {
    const f = this._field(name);
    if (f.kind !== ElementKind.BOOL) throw new MarshalError(Errno.EINVAL, `struct: field '${name}' is not bool`);
    return this._view.getUint32(AT(f.off), true) !== 0;
  }

  u8(name: string): u8 {
    const f = this._field(name);
    if (f.kind !== ElementKind.U8) throw new MarshalError(Errno.EINVAL, `struct: field '${name}' is not u8`);
    return this._view.getUint32(AT(f.off), true) as u8;
  }
  i8(name: string): i8 {
    const f = this._field(name);
    if (f.kind !== ElementKind.I8) throw new MarshalError(Errno.EINVAL, `struct: field '${name}' is not i8`);
    return this._view.getInt32(AT(f.off), true) as i8;
  }
  u16(name: string): u16 {
    const f = this._field(name);
    if (f.kind !== ElementKind.U16) throw new MarshalError(Errno.EINVAL, `struct: field '${name}' is not u16`);
    return this._view.getUint32(AT(f.off), true) as u16;
  }
  i16(name: string): i16 {
    const f = this._field(name);
    if (f.kind !== ElementKind.I16) throw new MarshalError(Errno.EINVAL, `struct: field '${name}' is not i16`);
    return this._view.getInt32(AT(f.off), true) as i16;
  }
  u32(name: string): u32 {
    const f = this._field(name);
    if (f.kind !== ElementKind.U32) throw new MarshalError(Errno.EINVAL, `struct: field '${name}' is not u32`);
    return this._view.getUint32(AT(f.off), true) as u32;
  }
  i32(name: string): i32 {
    const f = this._field(name);
    if (f.kind !== ElementKind.I32) throw new MarshalError(Errno.EINVAL, `struct: field '${name}' is not i32`);
    return this._view.getInt32(AT(f.off), true) as i32;
  }
  u64(name: string): u64 {
    const f = this._field(name);
    if (f.kind !== ElementKind.U64) throw new MarshalError(Errno.EINVAL, `struct: field '${name}' is not u64`);
    return this._view.getBigUint64(AT(f.off), true) as u64;
  }
  i64(name: string): i64 {
    const f = this._field(name);
    if (f.kind !== ElementKind.I64) throw new MarshalError(Errno.EINVAL, `struct: field '${name}' is not i64`);
    return this._view.getBigInt64(AT(f.off), true) as i64;
  }
  f32(name: string): f32 {
    const f = this._field(name);
    if (f.kind !== ElementKind.F32) throw new MarshalError(Errno.EINVAL, `struct: field '${name}' is not f32`);
    return this._view.getFloat32(AT(f.off), true) as f32;
  }
  f64(name: string): f64 {
    const f = this._field(name);
    if (f.kind !== ElementKind.F64) throw new MarshalError(Errno.EINVAL, `struct: field '${name}' is not f64`);
    return this._view.getFloat64(AT(f.off), true) as f64;
  }

  // ---- Ref readers ----

  str(name: string): string {
    const f = this._field(name);
    if (f.kind !== ElementKind.STR) throw new MarshalError(Errno.EINVAL, `struct: field '${name}' is not str`);
    const r = this._ref_at(f.off);
    return encoding.td.decode(this._buf.subarray(r.off, r.off + r.len));
  }

  u8_array(name: string): U8Array {
    const r = this._array_ref_at(name, ElementKind.U8_ARRAY);
    return new Uint8Array(this._buf.buffer, this._buf.byteOffset + r.off, r.len);
  }
  i8_array(name: string): I8Array {
    const r = this._array_ref_at(name, ElementKind.I8_ARRAY);
    return new Int8Array(this._buf.buffer, this._buf.byteOffset + r.off, r.len);
  }
  u16_array(name: string): U16Array {
    const r = this._array_ref_at(name, ElementKind.U16_ARRAY);
    return new Uint16Array(this._buf.buffer, this._buf.byteOffset + r.off, r.len);
  }
  i16_array(name: string): I16Array {
    const r = this._array_ref_at(name, ElementKind.I16_ARRAY);
    return new Int16Array(this._buf.buffer, this._buf.byteOffset + r.off, r.len);
  }
  u32_array(name: string): U32Array {
    const r = this._array_ref_at(name, ElementKind.U32_ARRAY);
    return new Uint32Array(this._buf.buffer, this._buf.byteOffset + r.off, r.len);
  }
  i32_array(name: string): I32Array {
    const r = this._array_ref_at(name, ElementKind.I32_ARRAY);
    return new Int32Array(this._buf.buffer, this._buf.byteOffset + r.off, r.len);
  }
  u64_array(name: string): U64Array {
    const r = this._array_ref_at(name, ElementKind.U64_ARRAY);
    return new BigUint64Array(this._buf.buffer, this._buf.byteOffset + r.off, r.len);
  }
  i64_array(name: string): I64Array {
    const r = this._array_ref_at(name, ElementKind.I64_ARRAY);
    return new BigInt64Array(this._buf.buffer, this._buf.byteOffset + r.off, r.len);
  }
  f32_array(name: string): F32Array {
    const r = this._array_ref_at(name, ElementKind.F32_ARRAY);
    return new Float32Array(this._buf.buffer, this._buf.byteOffset + r.off, r.len);
  }
  f64_array(name: string): F64Array {
    const r = this._array_ref_at(name, ElementKind.F64_ARRAY);
    return new Float64Array(this._buf.buffer, this._buf.byteOffset + r.off, r.len);
  }

  struct(name: string): StructReader {
    const f = this._field(name);
    if (f.kind !== ElementKind.STRUCT) throw new MarshalError(Errno.EINVAL, `struct: field '${name}' is not struct`);

    const off = this._struct_ref_at(f.off);
    const child = this._buf.subarray(off);
    const h = assert_header(child);
    return new StructReader(f.schema, this._buf.subarray(off, off + h.nbyte));
  }
}

// ============ Debug ============

function kind_name(kind: ElementKind): string {
  switch (kind) {
    case ElementKind.BOOL:
      return "bool";
    case ElementKind.U8:
      return "u8";
    case ElementKind.I8:
      return "i8";
    case ElementKind.U16:
      return "u16";
    case ElementKind.I16:
      return "i16";
    case ElementKind.U32:
      return "u32";
    case ElementKind.I32:
      return "i32";
    case ElementKind.U64:
      return "u64";
    case ElementKind.I64:
      return "i64";
    case ElementKind.F32:
      return "f32";
    case ElementKind.F64:
      return "f64";
    case ElementKind.STR:
      return "str";
    case ElementKind.U8_ARRAY:
      return "u8[]";
    case ElementKind.I8_ARRAY:
      return "i8[]";
    case ElementKind.U16_ARRAY:
      return "u16[]";
    case ElementKind.I16_ARRAY:
      return "i16[]";
    case ElementKind.U32_ARRAY:
      return "u32[]";
    case ElementKind.I32_ARRAY:
      return "i32[]";
    case ElementKind.U64_ARRAY:
      return "u64[]";
    case ElementKind.I64_ARRAY:
      return "i64[]";
    case ElementKind.F32_ARRAY:
      return "f32[]";
    case ElementKind.F64_ARRAY:
      return "f64[]";
    case ElementKind.STRUCT:
      return "struct";
  }
}

export function repr(schema: StructSchema): string {
  const out: string[] = [];
  out.push(`struct ${schema.name ?? "<anon>"} size=${schema.size}`);
  for (let i = 0; i < schema.fields.length; i++) {
    const f = schema.fields[i];
    if (f.kind === ElementKind.STRUCT) {
      out.push(`  +${f.off} ${f.name}: struct ${f.schema.name ?? "<anon>"}`);
    } else {
      out.push(`  +${f.off} ${f.name}: ${kind_name(f.kind)}`);
    }
  }
  return out.join("\n");
}
