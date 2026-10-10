// @pxl/shared/record/palette.ts

/**
 * The palette: one global record naming every kind of thing a placement
 * can be. A placement's `value` is a row here.
 *
 * A row for a cube is a colour; a row for a decor kind or an actor points
 * at its model by content — the glb's sha256 names the object in the
 * bucket, so the palette row is the directory entry and the model never
 * changes under it. A model whose texture is a separate image (the Kenney
 * packs share one colormap per pack) names that too, by the same rule;
 * a self-contained glb leaves it empty. `extent` is measured at ingest and
 * is what the range criterion reads; `license` and `author` are what
 * attribution shows. A user-uploaded kind is a row from USER_ID_BASE up
 * (issue/20260913-everything-is-a-file-the-placement-and-the-palette.md).
 */

import type { f32, u32 } from "@xpute/core/kit/word.ts";
import type { F32Array, U32Array, U64Array, U8Array } from "@xpute/core/kit/array.ts";
import type { BranchCursor } from "@xpute/core/io/marshal/tree/mod.ts";

import { encode_table, type RecordSource, str_at, Table } from "./table.ts";

export const PALETTE_KIND = "palette";
export const PALETTE_VERSION: u32 = 2;

/** What a row is, as the enum column stores it. */
export const PALETTE_KINDS = ["cube", "ground", "decor", "actor"] as const;
export type PaletteKind = (typeof PALETTE_KINDS)[number];

/** Rows issued for user uploads start here; below is the shipped set. */
export const USER_ID_BASE: u32 = 1 << 20;

export const PALETTE_KEYS = ["id", "kind", "name", "model_hash", "model_key", "skin_key", "extent", "size", "license", "author", "color", "group"] as const;

export type PaletteEntry = {
  id: u32;
  kind: PaletteKind;
  name: string;
  /** FNV-1a of the model's content hash; 0 for a kind with no model. */
  model_hash: bigint;
  /** The bucket key, `asset/<sha256>`; empty for a kind with no model. */
  model_key: string;
  /** The bucket key of the model's texture when it is a separate image;
   * empty for a self-contained glb or a kind with no model. */
  skin_key?: string;
  /** The model's bounds in world units (x, y, z), measured at ingest. */
  extent: readonly [f32, f32, f32];
  /** The footprint's side in world units, a power of two. */
  size: f32;
  license: string;
  author: string;
  /** A cube's colour, `#rrggbb`; empty for a kind drawn from a model. */
  color?: string;
  /** The scatter group a model belongs to — "tree", "car" — so a kind's
   * variants are found by name rather than by remembering ids. */
  group?: string;
};

export function encode_palette(rows: readonly PaletteEntry[]): U8Array {
  const n = rows.length;
  const id = new Uint32Array(n), kind = new Uint8Array(n), model_hash = new BigUint64Array(n);
  const extent = new Float32Array(n * 3), size = new Float32Array(n);
  rows.forEach((r, i) => {
    id[i] = r.id;
    kind[i] = PALETTE_KINDS.indexOf(r.kind);
    model_hash[i] = r.model_hash;
    extent.set(r.extent, i * 3);
    size[i] = r.size;
  });
  return encode_table(PALETTE_KIND, PALETTE_VERSION, n, [
    { key: "id", value: id, required: true },
    { key: "kind", value: kind, required: true },
    { key: "name", value: rows.map((r) => r.name), required: true },
    { key: "model_hash", value: model_hash, required: true },
    { key: "model_key", value: rows.map((r) => r.model_key), required: true },
    { key: "skin_key", value: rows.map((r) => r.skin_key ?? ""), required: false },
    { key: "extent", value: extent, required: true, stride: 3 },
    { key: "size", value: size, required: true },
    { key: "license", value: rows.map((r) => r.license), required: false },
    { key: "author", value: rows.map((r) => r.author), required: false },
    { key: "color", value: rows.map((r) => r.color ?? ""), required: false },
    { key: "group", value: rows.map((r) => r.group ?? ""), required: false },
  ]);
}

/** A palette opened over its packet, with a row index by id. */
export class PaletteTable {
  readonly count: u32;
  readonly id: U32Array;
  readonly kind: U8Array;
  readonly model_hash: U64Array;
  /** Three per row. */
  readonly extent: F32Array;
  readonly size: F32Array;
  private readonly name: BranchCursor;
  private readonly model_key: BranchCursor;
  private readonly skin_key: BranchCursor | null;
  private readonly license: BranchCursor | null;
  private readonly author: BranchCursor | null;
  private readonly color: BranchCursor | null;
  private readonly group: BranchCursor | null;
  private readonly by_id = new Map<u32, u32>();

  constructor(source: RecordSource) {
    const t = new Table(source, PALETTE_KEYS);
    if (t.kind !== PALETTE_KIND) throw new Error(`not a palette record: ${t.kind}`);
    this.count = t.count;
    this.id = t.lanes<U32Array>("id");
    this.kind = t.lanes<U8Array>("kind");
    this.model_hash = t.lanes<U64Array>("model_hash");
    this.extent = t.lanes<F32Array>("extent");
    this.size = t.lanes<F32Array>("size");
    this.name = t.strings("name")!;
    this.model_key = t.strings("model_key")!;
    this.skin_key = t.strings("skin_key");
    this.license = t.strings("license");
    this.author = t.strings("author");
    this.color = t.strings("color");
    this.group = t.strings("group");
    for (let i = 0; i < this.count; i++) this.by_id.set(this.id[i], i);
  }

  /** The rows of a scatter group, in declaration order — a kind's variants. */
  rows_of_group(group: string): u32[] {
    const out: u32[] = [];
    if (!this.group) return out;
    for (let i = 0; i < this.count; i++) if (str_at(this.group, i) === group) out.push(i);
    return out;
  }

  /** The row for a palette id, or -1. */
  row_of(id: u32): number {
    return this.by_id.get(id) ?? -1;
  }
  kind_of(i: u32): PaletteKind {
    return PALETTE_KINDS[this.kind[i]];
  }
  name_of(i: u32): string {
    return str_at(this.name, i);
  }
  model_key_of(i: u32): string {
    return str_at(this.model_key, i);
  }
  skin_key_of(i: u32): string {
    return this.skin_key ? str_at(this.skin_key, i) : "";
  }
  license_of(i: u32): string {
    return this.license ? str_at(this.license, i) : "";
  }
  author_of(i: u32): string {
    return this.author ? str_at(this.author, i) : "";
  }
  color_of(i: u32): string {
    return this.color ? str_at(this.color, i) : "";
  }
  group_of(i: u32): string {
    return this.group ? str_at(this.group, i) : "";
  }
  /** The largest dimension of the model — what the range criterion reads. */
  extent_max(i: u32): f32 {
    return Math.max(this.extent[i * 3], this.extent[i * 3 + 1], this.extent[i * 3 + 2]);
  }

  entry(i: u32): PaletteEntry {
    return {
      id: this.id[i],
      kind: this.kind_of(i),
      name: this.name_of(i),
      model_hash: this.model_hash[i],
      model_key: this.model_key_of(i),
      skin_key: this.skin_key_of(i) || undefined,
      extent: [this.extent[i * 3], this.extent[i * 3 + 1], this.extent[i * 3 + 2]],
      size: this.size[i],
      license: this.license_of(i),
      author: this.author_of(i),
      color: this.color_of(i) || undefined,
      group: this.group_of(i) || undefined,
    };
  }
}
