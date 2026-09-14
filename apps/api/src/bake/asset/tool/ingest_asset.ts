// @pxl/api/bake/asset/tool/ingest_asset.ts
//
// Ingests every shipped asset — the texture pack under asset/cube, the
// model packs under asset/decor and asset/actor — into the form the bucket
// holds and the palette names: one object per file, keyed by the SHA-256
// of its bytes, with a model's bounds measured here rather than typed in.
//
// Two outputs. ./out/asset/{glb,png,webp}/<sha256> is a copy of every file
// under its key, split by type so `aws s3 sync` can set one content-type per
// directory (the object name carries no extension, so the type has to be
// stored with it). And packages/pxl/shared/src/record/ingested.ts is the
// table the shipped palette reads — asset path to key, size and extent —
// generated in sorted order so a re-run with no changes writes no diff,
// and crates/pxl/shared/src/record/ingested.rs is the same table for the
// Rust pair, from the same rows.
//
// The texture pack is also ingested as one object: every image under
// asset/cube/{terrain,block} composed into a sheet, encoded once as WebP,
// with the tile order recorded beside the table. The client's layer array
// is filled from that one download rather than from sixty — the pack is
// one file, which is what a pack is.
//
// Run with:
//   deno run -A apps/api/src/bake/asset/tool/ingest_asset.ts
//
// A glb's extent is the size of its axis-aligned bounds over every node in
// every scene, node transforms applied, read from the POSITION accessors'
// own min/max (which glTF requires) — the same bounds a runtime loader
// would compute, so the number the range criterion reads is the one the
// mesh actually has. glTF is y-up; the extent is recorded in the world's
// ENU (x, y, z-up), which is (dx, dz, dy).

import { relative, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { Buffer } from "node:buffer";
import sharp from "npm:sharp@0.33";

import { render_rust_table, render_table, type Sheet } from "../table.ts";

const ROOT = fileURLToPath(new URL("../../../../../../", import.meta.url));
const ASSET_DIR = resolve(ROOT, "asset");
const OUT_DIR = resolve(ROOT, "out/asset");
const TABLE_PATH = resolve(ROOT, "packages/pxl/shared/src/record/ingested.ts");
const RUST_TABLE_PATH = resolve(ROOT, "crates/pxl/shared/src/record/ingested.rs");

async function* walk_files(dir: string): AsyncGenerator<string> {
  for await (const entry of Deno.readDir(dir)) {
    const path = resolve(dir, entry.name);
    if (entry.isDirectory) yield* walk_files(path);
    else if (entry.isFile) yield path;
  }
}

/** What the bucket prefix for content-addressed objects is; a key is this
 * over the sha256 hex. Mirrored by the Worker's KEY_RE and by
 * lib/env/route.ts's asset_url. */
const KEY_PREFIX = "asset/";

type Ingested = {
  path: string;
  key: string;
  bytes: number;
  type: "glb" | "png" | "webp";
  extent: [number, number, number] | null;
};

/** The pack as one image: `columns` tiles of `cell` pixels per row, tile n
 * at column n % columns, row n / columns; `tiles` is the asset path of each
 * tile in order. */

async function sha256_hex(bytes: Uint8Array): Promise<string> {
  const digest = await crypto.subtle.digest("SHA-256", bytes as BufferSource);
  return Array.from(new Uint8Array(digest), (b) => b.toString(16).padStart(2, "0")).join("");
}

// ---- glb bounds ----

type Mat4 = Float64Array; // column-major, 16

function mat4_identity(): Mat4 {
  const m = new Float64Array(16);
  m[0] =
    m[5] =
    m[10] =
    m[15] =
      1;
  return m;
}

function mat4_mul(a: Mat4, b: Mat4): Mat4 {
  const out = new Float64Array(16);
  for (let c = 0; c < 4; c++) {
    for (let r = 0; r < 4; r++) {
      let s = 0;
      for (let k = 0; k < 4; k++) s += a[k * 4 + r] * b[c * 4 + k];
      out[c * 4 + r] = s;
    }
  }
  return out;
}

function mat4_from_trs(t: number[] = [0, 0, 0], q: number[] = [0, 0, 0, 1], s: number[] = [1, 1, 1]): Mat4 {
  const [x, y, z, w] = q;
  const xx = x * x, yy = y * y, zz = z * z, xy = x * y, xz = x * z, yz = y * z, wx = w * x, wy = w * y, wz = w * z;
  const m = new Float64Array(16);
  m[0] = (1 - 2 * (yy + zz)) * s[0];
  m[1] = (2 * (xy + wz)) * s[0];
  m[2] = (2 * (xz - wy)) * s[0];
  m[4] = (2 * (xy - wz)) * s[1];
  m[5] = (1 - 2 * (xx + zz)) * s[1];
  m[6] = (2 * (yz + wx)) * s[1];
  m[8] = (2 * (xz + wy)) * s[2];
  m[9] = (2 * (yz - wx)) * s[2];
  m[10] = (1 - 2 * (xx + yy)) * s[2];
  m[12] = t[0];
  m[13] = t[1];
  m[14] = t[2];
  m[15] = 1;
  return m;
}

function transform_point(m: Mat4, p: number[]): [number, number, number] {
  return [
    m[0] * p[0] + m[4] * p[1] + m[8] * p[2] + m[12],
    m[1] * p[0] + m[5] * p[1] + m[9] * p[2] + m[13],
    m[2] * p[0] + m[6] * p[1] + m[10] * p[2] + m[14],
  ];
}

type Gltf = {
  scenes?: { nodes?: number[] }[];
  scene?: number;
  nodes?: { children?: number[]; mesh?: number; matrix?: number[]; translation?: number[]; rotation?: number[]; scale?: number[] }[];
  meshes?: { primitives: { attributes: Record<string, number> }[] }[];
  accessors?: { min?: number[]; max?: number[] }[];
};

function glb_json(bytes: Uint8Array): Gltf {
  const view = new DataView(bytes.buffer, bytes.byteOffset, bytes.byteLength);
  if (view.getUint32(0, true) !== 0x46546c67) throw new Error("not a glb");
  const chunk_len = view.getUint32(12, true);
  const chunk_type = view.getUint32(16, true);
  if (chunk_type !== 0x4e4f534a) throw new Error("first chunk is not JSON");
  return JSON.parse(new TextDecoder().decode(bytes.subarray(20, 20 + chunk_len)));
}

/** The glb's extent in ENU: (dx, dz, dy) of its world-space bounds. */
function glb_extent(bytes: Uint8Array): [number, number, number] {
  const gltf = glb_json(bytes);
  const lo = [Infinity, Infinity, Infinity], hi = [-Infinity, -Infinity, -Infinity];
  const visit = (index: number, parent: Mat4): void => {
    const node = gltf.nodes![index];
    const local = node.matrix ? Float64Array.from(node.matrix) : mat4_from_trs(node.translation, node.rotation, node.scale);
    const world = mat4_mul(parent, local);
    if (node.mesh !== undefined) {
      for (const primitive of gltf.meshes![node.mesh].primitives) {
        const accessor = gltf.accessors![primitive.attributes.POSITION];
        if (!accessor?.min || !accessor?.max) continue;
        for (let corner = 0; corner < 8; corner++) {
          const p = [
            corner & 1 ? accessor.max[0] : accessor.min[0],
            corner & 2 ? accessor.max[1] : accessor.min[1],
            corner & 4 ? accessor.max[2] : accessor.min[2],
          ];
          const w = transform_point(world, p);
          for (let i = 0; i < 3; i++) {
            lo[i] = Math.min(lo[i], w[i]);
            hi[i] = Math.max(hi[i], w[i]);
          }
        }
      }
    }
    for (const child of node.children ?? []) visit(child, world);
  };
  const scenes = gltf.scene !== undefined ? [gltf.scenes![gltf.scene]] : gltf.scenes ?? [];
  for (const scene of scenes) for (const root of scene.nodes ?? []) visit(root, mat4_identity());
  if (!Number.isFinite(lo[0])) throw new Error("glb has no positioned geometry");
  const round = (v: number) => Math.round(v * 100) / 100;
  return [round(hi[0] - lo[0]), round(hi[2] - lo[2]), round(hi[1] - lo[1])];
}

// ---- main ----

async function ingest(): Promise<Ingested[]> {
  const rows: Ingested[] = [];
  for await (const file of walk_files(ASSET_DIR)) {
    if (!file.endsWith(".glb") && !file.endsWith(".png")) continue;
    const path = relative(ASSET_DIR, file).replaceAll("\\", "/");
    // The native-resolution texture originals are the resize source for
    // download_texture_pack.ts, and the brand sources are render_brand.ts's
    // — neither is something the client fetches by key.
    if (path.startsWith("cube/original/") || path.startsWith("brand/")) continue;
    const bytes = await Deno.readFile(file);
    const type = path.endsWith(".glb") ? "glb" : "png";
    const hex = await sha256_hex(bytes);
    rows.push({ path, key: KEY_PREFIX + hex, bytes: bytes.byteLength, type, extent: type === "glb" ? glb_extent(bytes) : null });
    await Deno.mkdir(resolve(OUT_DIR, type), { recursive: true });
    await Deno.writeFile(resolve(OUT_DIR, type, hex), bytes);
  }
  rows.sort((a, b) => a.path < b.path ? -1 : a.path > b.path ? 1 : 0);
  return rows;
}

const SHEET_CELL = 256;
const SHEET_COLUMNS = 8;

/** Composes the pack's images into one WebP sheet and writes it under its
 * own key. Tiles are the pack's images in path order, each checked to be
 * one cell square — the same normalized size download_texture_pack.ts
 * writes. */
async function ingest_sheet(rows: Ingested[]): Promise<Sheet> {
  const tiles = rows.filter((r) => r.type === "png" && (r.path.startsWith("cube/terrain/") || r.path.startsWith("cube/block/"))).map((r) => r.path);
  const rows_n = Math.ceil(tiles.length / SHEET_COLUMNS);
  const composite = [];
  for (let n = 0; n < tiles.length; n++) {
    const file = resolve(ASSET_DIR, tiles[n]);
    const meta = await sharp(file).metadata();
    if (meta.width !== SHEET_CELL || meta.height !== SHEET_CELL) throw new Error(`ingest: ${tiles[n]} is ${meta.width}x${meta.height}, not ${SHEET_CELL} square`);
    composite.push({ input: file, left: (n % SHEET_COLUMNS) * SHEET_CELL, top: Math.floor(n / SHEET_COLUMNS) * SHEET_CELL });
  }
  const bytes = new Uint8Array(
    await sharp({ create: { width: SHEET_COLUMNS * SHEET_CELL, height: rows_n * SHEET_CELL, channels: 4, background: { r: 0, g: 0, b: 0, alpha: 1 } } })
      .composite(composite)
      .webp({ quality: 92 })
      .toBuffer(),
  );
  const hex = await sha256_hex(bytes);
  await Deno.mkdir(resolve(OUT_DIR, "webp"), { recursive: true });
  await Deno.writeFile(resolve(OUT_DIR, "webp", hex), bytes);
  return { key: KEY_PREFIX + hex, bytes: bytes.byteLength, cell: SHEET_CELL, columns: SHEET_COLUMNS, tiles };
}

const rows = await ingest();
const sheet = await ingest_sheet(rows);
await Deno.writeTextFile(TABLE_PATH, render_table(rows, sheet));
await Deno.writeTextFile(RUST_TABLE_PATH, render_rust_table(rows, sheet));
const total = rows.reduce((n, r) => n + r.bytes, 0);
console.log(
  `[ingest] ${rows.length} assets, ${(total / 1024 / 1024).toFixed(1)} MiB, sheet ${sheet.tiles.length} tiles ${(sheet.bytes / 1024).toFixed(0)} KiB -> ${
    relative(ROOT, OUT_DIR)
  }/{glb,png,webp}, table ${relative(ROOT, TABLE_PATH)}`,
);
