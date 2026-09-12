// @pxl/shared/record/placement.ts

/**
 * The Placement: one record for one placed thing, whatever it is.
 *
 * A tree, a placed cube, a car, a bare ground cell — every thing drawn at
 * a place is a row of this table, and what differs between them is the
 * palette entry `value` names, not the row
 * (issue/20260913-everything-is-a-file-the-placement-and-the-palette.md).
 *
 * Positions are page-local world units, as the POI page's are; `heading`
 * is radians about the up axis, counter-clockwise from +x, in the ENU
 * frame every model is converted into at the glTF boundary. `id` is the
 * FNV-1a of the thing's address, or of its external id where it has one.
 * `scale`, `rotation` and `tags` are ancillary: a reader that does not
 * know them skips them, and a record need not carry them.
 */

import type { f32, u32 } from "@xpute/core/kit/word.ts";
import type { F32Array, U32Array, U64Array, U8Array } from "@xpute/core/kit/array.ts";
import type { BranchCursor } from "@xpute/core/io/marshal/tree/mod.ts";

import { type Column, encode_table, type RecordSource, str_at, Table } from "./table.ts";

export const PLACEMENT_KIND = "placement";
export const PLACEMENT_VERSION: u32 = 1;

/** The columns a placement reader understands, required ones first. */
export const PLACEMENT_KEYS = ["id", "x", "y", "z", "size", "heading", "value", "scale", "rotation", "tags"] as const;

export type Placement = {
  id: bigint;
  x: f32;
  y: f32;
  z: f32;
  /** The footprint's side in world units — a cube 1, a sub-cube 1/4, a
   * two-cell building 2. A power of two by construction. */
  size: f32;
  heading: f32;
  value: u32;
  scale?: f32;
  /** Full orientation as a quaternion (x, y, z, w), when a thing has one. */
  rotation?: readonly [f32, f32, f32, f32];
  tags?: Record<string, string | undefined>;
};

/** Encodes rows. An ancillary column is written only if some row has it;
 * a row without it gets the neutral value (scale 1, identity rotation, no
 * tags). */
export function encode_placements(rows: readonly Placement[]): U8Array {
  const n = rows.length;
  const id = new BigUint64Array(n), x = new Float32Array(n), y = new Float32Array(n), z = new Float32Array(n);
  const size = new Float32Array(n), heading = new Float32Array(n), value = new Uint32Array(n);
  let any_scale = false, any_rotation = false, any_tags = false;
  for (let i = 0; i < n; i++) {
    const r = rows[i];
    id[i] = r.id;
    x[i] = r.x;
    y[i] = r.y;
    z[i] = r.z;
    size[i] = r.size;
    heading[i] = r.heading;
    value[i] = r.value;
    if (r.scale !== undefined) any_scale = true;
    if (r.rotation) any_rotation = true;
    if (r.tags) any_tags = true;
  }
  const columns: Column[] = [
    { key: "id", value: id, required: true },
    { key: "x", value: x, required: true },
    { key: "y", value: y, required: true },
    { key: "z", value: z, required: true },
    { key: "size", value: size, required: true },
    { key: "heading", value: heading, required: true },
    { key: "value", value, required: true },
  ];
  if (any_scale) {
    const scale = new Float32Array(n).fill(1);
    rows.forEach((r, i) => (scale[i] = r.scale ?? 1));
    columns.push({ key: "scale", value: scale, required: false });
  }
  if (any_rotation) {
    const rotation = new Float32Array(n * 4);
    rows.forEach((r, i) => rotation.set(r.rotation ?? [0, 0, 0, 1], i * 4));
    columns.push({ key: "rotation", value: rotation, required: false, stride: 4 });
  }
  if (any_tags) {
    columns.push({ key: "tags", value: rows.map((r) => (r.tags ? JSON.stringify(r.tags) : "")), required: false });
  }
  return encode_table(PLACEMENT_KIND, PLACEMENT_VERSION, n, columns);
}

/** A placement record opened over its packet: the lanes are views, the
 * optional ones null when the record does not carry them. */
export class PlacementTable {
  readonly count: u32;
  readonly id: U64Array;
  readonly x: F32Array;
  readonly y: F32Array;
  readonly z: F32Array;
  readonly size: F32Array;
  readonly heading: F32Array;
  readonly value: U32Array;
  readonly scale: F32Array | null;
  /** Four per row. */
  readonly rotation: F32Array | null;
  private readonly tags: BranchCursor | null;

  constructor(source: RecordSource) {
    const t = new Table(source, PLACEMENT_KEYS);
    if (t.kind !== PLACEMENT_KIND) throw new Error(`not a placement record: ${t.kind}`);
    this.count = t.count;
    this.id = t.lanes<U64Array>("id");
    this.x = t.lanes<F32Array>("x");
    this.y = t.lanes<F32Array>("y");
    this.z = t.lanes<F32Array>("z");
    this.size = t.lanes<F32Array>("size");
    this.heading = t.lanes<F32Array>("heading");
    this.value = t.lanes<U32Array>("value");
    this.scale = t.lanes_opt<F32Array>("scale");
    this.rotation = t.lanes_opt<F32Array>("rotation");
    this.tags = t.strings("tags");
  }

  scale_of(i: u32): f32 {
    return this.scale ? this.scale[i] : 1;
  }

  tags_of(i: u32): Record<string, string | undefined> | undefined {
    if (!this.tags) return undefined;
    const s = str_at(this.tags, i);
    return s === "" ? undefined : JSON.parse(s);
  }

  /** The row as the object form — a detail view, a test. One allocation
   * per ask, never per record. */
  row(i: u32): Placement {
    const out: Placement = {
      id: this.id[i],
      x: this.x[i],
      y: this.y[i],
      z: this.z[i],
      size: this.size[i],
      heading: this.heading[i],
      value: this.value[i],
    };
    if (this.scale) out.scale = this.scale[i];
    if (this.rotation) out.rotation = [this.rotation[i * 4], this.rotation[i * 4 + 1], this.rotation[i * 4 + 2], this.rotation[i * 4 + 3]];
    const tags = this.tags_of(i);
    if (tags) out.tags = tags;
    return out;
  }
}
