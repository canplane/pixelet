// @pxl/web/render/layer/pipeline/chunk_mesher.ts

/**
 * Pure 64x64 chunk -> baked GPU mesh.
 *
 * Bakes real per-vertex geometry directly, one merged mesh per chunk (cube
 * terrain AND cube objects together — never instanced, never split into two
 * meshes): every surviving run contributes its own box's actual vertices
 * into one growing set of typed arrays, the same "each block bakes its own
 * geometry into one shared per-chunk buffer" model Minecraft itself uses,
 * chosen specifically because it's geometry-agnostic — decor_layer.tsx's own
 * arbitrary GLB geometry can bake into this exact same shape of output
 * (position/normal + a handful of per-vertex material attributes), which a
 * shared-box-instanced pipeline could never support. See
 * discussions/20260903-terrain-decor-render-unification.md for the fuller
 * account of why this replaced the previous instanced-box approach.
 *
 * Every box here is still axis-aligned scale+translation only, exactly like
 * the instanced approach this replaces (chunk_raycast.ts's boxes still come
 * from the same run_to_box contract) — so face normals need no per-box
 * transform at all (a diagonal scale never rotates an axis-aligned unit
 * normal), and UV/bevel math only needs each vertex's own canonical
 * box-local corner (`a_local_pos`, [-0.5, 0.5]^3) plus that box's own real
 * per-axis size (`a_box_scale`) — both baked once per vertex here and read
 * directly by chunk_stage.tsx's vertex shader, no matrix decomposition
 * required there anymore.
 */

import type { f64, i32, u32 } from "@xpute/core/kit/word.ts";
import { CHUNK_CELL_SPAN, U_PER_CELL } from "@pxl/shared/bake/constants.ts";

import type { LodStep } from "@/render/lod.ts";
import { MaterialClass, NO_LAYER, SUB_CELL_NONE, unpack_sub_cell_fraction } from "./volume.ts";
import { type MeshRun, run_to_box } from "./voxel_scan.ts";

/**
 * One baked chunk mesh's raw vertex data — position/normal are the real
 * chunk-local vertices every box's own faces actually sit at; the rest are
 * per-vertex material/shading inputs chunk_stage.tsx's shader reads
 * directly (see this module's own doc comment for why `local_pos`/
 * `box_scale` replace what an instanced approach would reconstruct from
 * `instanceMatrix`). `visibility` is deliberately NOT included here — same
 * layering as before: this module only ever bakes real geometry, never a
 * per-role default (chunk_bucket.ts's own with_visibility still owns that,
 * just sized by vertex_cnt now instead of instance cnt).
 */
export type BakedChunkMesh = {
  vertex_cnt: u32;
  /** vertex_cnt*3 floats: real chunk-local position. */
  position: Float32Array;
  /** vertex_cnt*3 floats: canonical face normal (never scaled — see doc comment). */
  normal: Float32Array;
  /** vertex_cnt*3 floats: this vertex's own box-local corner, [-0.5, 0.5]^3. */
  local_pos: Float32Array;
  /** vertex_cnt*3 floats: this vertex's own box's real per-axis size. */
  box_scale: Float32Array;
  /** vertex_cnt*3 floats, linear-space RGB. */
  rgb: Float32Array;
  /** vertex_cnt*4 floats, one terrain texture layer per supersample tap. */
  layer_taps: Float32Array;
  /** Triangle index buffer (Uint32 — a dense chunk can comfortably exceed
   * Uint16's 65535-vertex ceiling once every box bakes 20-24 real vertices
   * instead of costing one shared 24-vertex buffer regardless of count). */
  index: Uint32Array;
};

/**
 * Dev-only, cumulative since page load: how many coarse-LOD instances this
 * session has written, and how many of those were low-fill enough
 * (`fill < LOW_FILL_THRESHOLD`) that a threshold-cull policy (drop instead
 * of shrink — see chunk_stage.tsx's lod_build_stats sibling) would have
 * skipped them outright. Answers "what does always-shrink-never-drop
 * actually cost" with a real count instead of a guess. Exposed the same
 * no-console way, via chunk_stage.tsx's __PXL_RENDER_STATS__.
 */
export const coarse_instance_stats = { total: 0, low_fill: 0 };
const LOW_FILL_THRESHOLD: f64 = 0.25;

/** Dev-only, cumulative since page load: LOD1 runs seen vs. how many were occlusion-culled (see is_run_occluded). */
export const occlusion_stats = { total_runs: 0, culled: 0 };

/** sRGB [0,1] -> linear [0,1]. */
function srgb_to_linear(c: f64): f64 {
  return c <= 0.04045 ? c / 12.92 : Math.pow((c + 0.055) / 1.055, 2.4);
}

// ---- Canonical unit-box face table ----
//
// A box centered at the origin, half-extent 0.5 on every axis (matches
// run_to_box's own min/size contract — emit_box below recenters+rescales
// these per box). Each face keeps its own 4 corners (never shared across
// faces, same reasoning three.js's own BoxGeometry keeps faces unshared: a
// shared vertex can't carry more than one face normal/UV). Corner winding
// (00,10,11,01 with triangles (00,10,11)/(00,11,01)) is CCW as seen from
// outside for every face, derived from face_normal = tangent_u x tangent_v.
// The -Z (bottom) face MUST stay last — emit_box conditionally drops it,
// mirroring the old TERRAIN_BOX/OBJECT_BOX open-bottom-vs-full split.
type Face = { normal: readonly [f64, f64, f64]; corners: readonly (readonly [f64, f64, f64])[] };
const FACES: readonly Face[] = [
  { normal: [1, 0, 0], corners: [[0.5, -0.5, -0.5], [0.5, 0.5, -0.5], [0.5, 0.5, 0.5], [0.5, -0.5, 0.5]] },
  { normal: [-1, 0, 0], corners: [[-0.5, -0.5, -0.5], [-0.5, -0.5, 0.5], [-0.5, 0.5, 0.5], [-0.5, 0.5, -0.5]] },
  { normal: [0, 1, 0], corners: [[-0.5, 0.5, -0.5], [-0.5, 0.5, 0.5], [0.5, 0.5, 0.5], [0.5, 0.5, -0.5]] },
  { normal: [0, -1, 0], corners: [[-0.5, -0.5, -0.5], [0.5, -0.5, -0.5], [0.5, -0.5, 0.5], [-0.5, -0.5, 0.5]] },
  { normal: [0, 0, 1], corners: [[-0.5, -0.5, 0.5], [0.5, -0.5, 0.5], [0.5, 0.5, 0.5], [-0.5, 0.5, 0.5]] },
  { normal: [0, 0, -1], corners: [[-0.5, -0.5, -0.5], [-0.5, 0.5, -0.5], [0.5, 0.5, -0.5], [0.5, -0.5, -0.5]] },
];
const FULL_FACE_CNT: u32 = FACES.length;
const OPEN_BOTTOM_FACE_CNT: u32 = FACES.length - 1;

/** Growing-by-preallocation vertex/index writer — callers precompute the
 * exact vertex/index totals (every emit_box call's own face count is known
 * ahead of time from include_bottom alone), so this never resizes. */
type MeshBuilder = {
  position: Float32Array;
  normal: Float32Array;
  local_pos: Float32Array;
  box_scale: Float32Array;
  rgb: Float32Array;
  layer_taps: Float32Array;
  index: Uint32Array;
  v_cnt: u32;
  i_cnt: u32;
};

function alloc_builder(vertex_total: u32, index_total: u32): MeshBuilder {
  return {
    position: new Float32Array(vertex_total * 3),
    normal: new Float32Array(vertex_total * 3),
    local_pos: new Float32Array(vertex_total * 3),
    box_scale: new Float32Array(vertex_total * 3),
    rgb: new Float32Array(vertex_total * 3),
    layer_taps: new Float32Array(vertex_total * 4),
    index: new Uint32Array(index_total),
    v_cnt: 0,
    i_cnt: 0,
  };
}

function finish_builder(builder: MeshBuilder): BakedChunkMesh {
  return {
    vertex_cnt: builder.v_cnt,
    position: builder.position,
    normal: builder.normal,
    local_pos: builder.local_pos,
    box_scale: builder.box_scale,
    rgb: builder.rgb,
    layer_taps: builder.layer_taps,
    index: builder.index,
  };
}

/** Bakes one box's own real vertices (5 faces, bottom dropped, or all 6) directly into `builder`. */
function emit_box(
  builder: MeshBuilder,
  min: readonly [f64, f64, f64],
  size: readonly [f64, f64, f64],
  r: f64,
  g: f64,
  b: f64,
  layer_taps: readonly [f64, f64, f64, f64],
  include_bottom: boolean,
): void {
  const face_cnt = include_bottom ? FULL_FACE_CNT : OPEN_BOTTOM_FACE_CNT;
  const cx = min[0] + size[0] * 0.5, cy = min[1] + size[1] * 0.5, cz = min[2] + size[2] * 0.5;

  for (let f: u32 = 0; f < face_cnt; f++) {
    const face = FACES[f];
    const base_v = builder.v_cnt;
    for (let c = 0; c < 4; c++) {
      const corner = face.corners[c];
      const vi = builder.v_cnt++;
      const po = vi * 3, lo = vi * 4;
      builder.position[po] = cx + corner[0] * size[0];
      builder.position[po + 1] = cy + corner[1] * size[1];
      builder.position[po + 2] = cz + corner[2] * size[2];
      builder.normal[po] = face.normal[0];
      builder.normal[po + 1] = face.normal[1];
      builder.normal[po + 2] = face.normal[2];
      builder.local_pos[po] = corner[0];
      builder.local_pos[po + 1] = corner[1];
      builder.local_pos[po + 2] = corner[2];
      builder.box_scale[po] = size[0];
      builder.box_scale[po + 1] = size[1];
      builder.box_scale[po + 2] = size[2];
      builder.rgb[po] = r;
      builder.rgb[po + 1] = g;
      builder.rgb[po + 2] = b;
      builder.layer_taps[lo] = layer_taps[0];
      builder.layer_taps[lo + 1] = layer_taps[1];
      builder.layer_taps[lo + 2] = layer_taps[2];
      builder.layer_taps[lo + 3] = layer_taps[3];
    }
    builder.index[builder.i_cnt++] = base_v;
    builder.index[builder.i_cnt++] = base_v + 1;
    builder.index[builder.i_cnt++] = base_v + 2;
    builder.index[builder.i_cnt++] = base_v;
    builder.index[builder.i_cnt++] = base_v + 2;
    builder.index[builder.i_cnt++] = base_v + 3;
  }
}

function column_covers_range(runs: readonly MeshRun[], z0: i32, z1: i32): boolean {
  for (const run of runs) {
    if (run.sub_sz !== SUB_CELL_NONE) continue;
    if (run.z0 <= z0 && run.z1 >= z1) return true;
  }
  return false;
}

/**
 * OBJECT-class runs never count as "covering" whatever's below them here —
 * a placed object sitting on a terrain cell doesn't form the kind of
 * continuous solid mass this occlusion check is meant to detect (a genuine
 * buried voxel, sealed on every side by more terrain/structure). The
 * camera is a near-top-down but still oblique view, not perfectly
 * vertical, so a small standalone object never actually blocks its
 * terrain cell's top face the way a real overlying layer would — treating
 * it as if it did was culling that terrain instance outright instead of
 * just letting the object visually sit in front of it, showing through as
 * a hole (bug found 2026-08-16: placing objects visibly dug into the
 * terrain beneath them).
 */
function is_topmost_run(runs: readonly MeshRun[], run: MeshRun): boolean {
  const top: i32 = run.sub_sz !== SUB_CELL_NONE ? run.z0 : run.z1;
  for (const other of runs) {
    if (other.material_class === MaterialClass.OBJECT) continue;
    const other_top: i32 = other.sub_sz !== SUB_CELL_NONE ? other.z0 : other.z1;
    if (other_top > top) return false;
  }
  return true;
}

/**
 * A run fully surrounded by opaque neighbors — all 4 horizontal neighbor
 * columns solid across its whole z0..z1 span, and something else (never an
 * OBJECT run — see is_topmost_run's own doc comment) in its own column
 * above it — can never show a face, so it costs nothing to bake. Chunk-edge
 * columns (cx/cy at 0 or CHUNK_CELL_SPAN-1) never cull: the neighboring
 * chunk's data isn't available here, and assuming it's solid would be a
 * real correctness risk (a visible hole) for a cosmetic win. Sub-cell
 * (fractional) runs never cull or get culled either — too thin, and exact
 * placement is the whole point of them.
 */
function is_run_occluded(column_runs: readonly MeshRun[][], cx: u32, cy: u32, run: MeshRun): boolean {
  if (run.sub_sz !== SUB_CELL_NONE) return false;
  if (cx === 0 || cx === CHUNK_CELL_SPAN - 1 || cy === 0 || cy === CHUNK_CELL_SPAN - 1) return false;
  const own = column_runs[cy * CHUNK_CELL_SPAN + cx];
  if (is_topmost_run(own, run)) return false;
  if (!column_covers_range(column_runs[cy * CHUNK_CELL_SPAN + (cx - 1)], run.z0, run.z1)) return false;
  if (!column_covers_range(column_runs[cy * CHUNK_CELL_SPAN + (cx + 1)], run.z0, run.z1)) return false;
  if (!column_covers_range(column_runs[(cy - 1) * CHUNK_CELL_SPAN + cx], run.z0, run.z1)) return false;
  if (!column_covers_range(column_runs[(cy + 1) * CHUNK_CELL_SPAN + cx], run.z0, run.z1)) return false;
  return true;
}

/** A cube run's own face count — full box for OBJECT-class runs (placed
 * objects can be viewed from underneath: stacked, on a ledge, camera
 * clipped below), open-bottom for everything else (terrain always sits on
 * more terrain or the chunk's own floor, so its bottom face is never
 * visible from any camera angle a player can reach) — exactly TERRAIN_BOX's
 * vs OBJECT_BOX's old distinction, now a per-run decision instead of a
 * per-mesh one, which is what lets terrain and object cubes bake into one
 * merged mesh instead of staying split into two. */
function run_include_bottom(run: MeshRun): boolean {
  return run.material_class === MaterialClass.OBJECT;
}

function run_face_cnt(run: MeshRun): u32 {
  return run_include_bottom(run) ? FULL_FACE_CNT : OPEN_BOTTOM_FACE_CNT;
}

/**
 * Pure and allocation-owning: each call returns one exactly-sized immutable
 * baked mesh for one 64x64 chunk and one LOD step, from a chunk's
 * `column_runs` (see `voxel_scan.ts#scan_all_column_runs`) — scanned once
 * from the live `ChunkVolume` and reusable across every LOD rebuild after
 * that, long after the volume itself is released back to its pool.
 */
export function bake_chunk_mesh(column_runs: readonly MeshRun[][], lod_step: LodStep): BakedChunkMesh {
  if (lod_step === 1) return bake_chunk_mesh_fine(column_runs);
  return bake_chunk_mesh_coarse(column_runs, lod_step);
}

function bake_chunk_mesh_fine(column_runs: readonly MeshRun[][]): BakedChunkMesh {
  const surviving: { cx: u32; cy: u32; run: MeshRun }[] = [];
  let vertex_total: u32 = 0;
  let index_total: u32 = 0;

  for (let cy: u32 = 0; cy < CHUNK_CELL_SPAN; cy++) {
    for (let cx: u32 = 0; cx < CHUNK_CELL_SPAN; cx++) {
      for (const run of column_runs[cy * CHUNK_CELL_SPAN + cx]) {
        occlusion_stats.total_runs++;
        if (is_run_occluded(column_runs, cx, cy, run)) {
          occlusion_stats.culled++;
          continue;
        }
        surviving.push({ cx, cy, run });
        const faces = run_face_cnt(run);
        vertex_total += faces * 4;
        index_total += faces * 6;
      }
    }
  }

  const builder = alloc_builder(vertex_total, index_total);
  for (const { cx, cy, run } of surviving) {
    const box = run_to_box(cx, cy, run);
    const size: [f64, f64, f64] = [box.max[0] - box.min[0], box.max[1] - box.min[1], box.max[2] - box.min[2]];
    emit_box(
      builder,
      box.min,
      size,
      srgb_to_linear(run.r / 255),
      srgb_to_linear(run.g / 255),
      srgb_to_linear(run.b / 255),
      [run.layer_idx, run.layer_idx, run.layer_idx, run.layer_idx],
      run_include_bottom(run),
    );
  }
  return finish_builder(builder);
}

/** layer_taps: up to 4 distinct texture-array layers touching this bucket,
 * first-encountered order (NO_LAYER — objects, always — never added). */
type ZBucketAccum = { bz: i32; pr: f64; pg: f64; pb: f64; coverage: f64; layer_taps: u32[] };

/**
 * Linear scan, not a Map: a coarse cell's real Z-bucket count is small
 * (bounded by how many bands actually stack under it), and a plain array
 * avoids allocating a hash-table container per cell — `bake_chunk_mesh_coarse`
 * calls this up to `res^2` times per build (1024x for `lod_step=2`), so the
 * per-cell container itself, not just its entries, has to stay cheap.
 */
function find_or_create_bucket(buckets: ZBucketAccum[], bz: i32): ZBucketAccum {
  for (let i = 0; i < buckets.length; i++) {
    if (buckets[i].bz === bz) return buckets[i];
  }
  const acc: ZBucketAccum = { bz, pr: 0, pg: 0, pb: 0, coverage: 0, layer_taps: [] };
  buckets.push(acc);
  return acc;
}

/** Same small-N linear-scan reasoning as find_or_create_bucket — a bucket
 * realistically sees a handful of distinct layers, not enough to justify a
 * hash table. Caps at 4 (matches layer_taps' fixed-size vec4 slot); beyond
 * that, whichever 4 were encountered first win. */
function collect_layer_tap(acc: ZBucketAccum, layer_idx: u32): void {
  if (layer_idx === NO_LAYER || acc.layer_taps.length >= 4 || acc.layer_taps.includes(layer_idx)) return;
  acc.layer_taps.push(layer_idx);
}

/**
 * Folds one fine-column run into whichever `lod_step`-tall Z buckets it
 * overlaps. Material class is intentionally ignored here — terrain and
 * objects are summed into the same average, on purpose (a coarse cell's
 * color is whatever's really in it, air included via `coverage` staying
 * below `lod_step^3`). A sub-cell run occupies one fine voxel fractionally;
 * its weight is the volumetric fraction (linear fraction cubed), not 1.
 */
function accumulate_run(buckets: ZBucketAccum[], lod_step: LodStep, run: MeshRun): void {
  if (run.sub_sz !== SUB_CELL_NONE) {
    const frac = unpack_sub_cell_fraction(run.sub_sz);
    const w = frac * frac * frac;
    if (w <= 0) return;
    const acc = find_or_create_bucket(buckets, Math.floor(run.z0 / lod_step));
    acc.pr += run.r * w;
    acc.pg += run.g * w;
    acc.pb += run.b * w;
    acc.coverage += w;
    collect_layer_tap(acc, run.layer_idx);
    return;
  }

  const bz_lo: i32 = Math.floor(run.z0 / lod_step);
  const bz_hi: i32 = Math.floor(run.z1 / lod_step);
  for (let bz = bz_lo; bz <= bz_hi; bz++) {
    const bucket_lo = bz * lod_step;
    const bucket_hi = bucket_lo + lod_step - 1;
    const overlap = Math.min(run.z1, bucket_hi) - Math.max(run.z0, bucket_lo) + 1;
    if (overlap <= 0) continue;
    const acc = find_or_create_bucket(buckets, bz);
    acc.pr += run.r * overlap;
    acc.pg += run.g * overlap;
    acc.pb += run.b * overlap;
    acc.coverage += overlap;
    collect_layer_tap(acc, run.layer_idx);
  }
}

/**
 * Bakes one averaged coarse-cell box. Fill fraction (`coverage /
 * lod_step^3`, in (0,1]) shrinks the box isotropically — X/Y/Z scale
 * together by the same cube-root factor — rather than rendering real alpha:
 * an opaque smaller box is visually "fades out" without transparency's
 * blend/sort cost. `acc.layer_taps` (up to 4 distinct layers, first
 * encountered, padded with NO_LAYER) become this box's texture taps, so the
 * texture-pack setting still shows *something* at coarse LOD instead of
 * always falling back to the flat averaged color. Always open-bottom (5
 * faces): a coarse box is always classified as merged "terrain" regardless
 * of how much OBJECT material contributed to its average color — same
 * behavior the old build_chunk_instances_coarse had (its own output always
 * rendered through TERRAIN_BOX, never OBJECT_BOX).
 */
function emit_averaged_box(builder: MeshBuilder, bx: u32, by: u32, lod_step: LodStep, acc: ZBucketAccum): void {
  const max_coverage: f64 = lod_step * lod_step * lod_step;
  const fill: f64 = Math.min(1, acc.coverage / max_coverage);
  coarse_instance_stats.total++;
  if (fill < LOW_FILL_THRESHOLD) coarse_instance_stats.low_fill++;
  const span: f64 = lod_step * U_PER_CELL;
  const sz: f64 = span * Math.cbrt(fill);
  const cz: f64 = (acc.bz * lod_step + lod_step * 0.5) * U_PER_CELL;
  const half = sz * 0.5;
  const min: [f64, f64, f64] = [(bx + 0.5) * span - half, (by + 0.5) * span - half, cz - half];
  const size: [f64, f64, f64] = [sz, sz, sz];

  // The shader always averages 4 taps at 25% each — padding unused slots
  // with NO_LAYER (falls back to the flat instance color) dilutes toward
  // flat color whenever fewer than 4 distinct layers touched this bucket,
  // which is the common case (most buckets are one material, not four).
  // Cycling the real taps into every slot instead keeps the blend 100%
  // texture-derived whenever at least one real layer exists, matching what
  // LOD1 already does (all 4 of its taps are the same one real layer_idx).
  const n = acc.layer_taps.length;
  const layer_taps: [f64, f64, f64, f64] = [
    n > 0 ? acc.layer_taps[0 % n] : NO_LAYER,
    n > 0 ? acc.layer_taps[1 % n] : NO_LAYER,
    n > 0 ? acc.layer_taps[2 % n] : NO_LAYER,
    n > 0 ? acc.layer_taps[3 % n] : NO_LAYER,
  ];

  emit_box(
    builder,
    min,
    size,
    srgb_to_linear(acc.pr / acc.coverage / 255),
    srgb_to_linear(acc.pg / acc.coverage / 255),
    srgb_to_linear(acc.pb / acc.coverage / 255),
    layer_taps,
    false,
  );
}

/**
 * Coarse LOD box-averages every fine column under each `lod_step`-wide XY
 * cell (isotropic — X/Y/Z all scale by the same `lod_step`, never Z alone),
 * binned into `lod_step`-tall Z buckets, terrain and objects summed into
 * one blended color per bucket. This is the mipmap/voxel-cone-tracing
 * screen-footprint-filtering idea, not nearest-neighbor: color mixing
 * across material boundaries is intentional here, and a cell that's partly
 * air renders as a proportionally smaller opaque box (see
 * emit_averaged_box) instead of popping to nothing at a hard occupancy
 * threshold. An object straddling a fixed cell boundary still splits across
 * two smaller boxes rather than one full one — an accepted side effect of a
 * fixed grid, not the old hard-threshold disappearing bug.
 *
 * No per-level pyramid is stored, but no rescanning happens either: every
 * `lod_step` is box-averaged from the same already-scanned `column_runs`
 * (see `bake_chunk_mesh`'s doc comment) — exhaustively, all `lod_step^2`
 * real columns per coarse cell, no subsampling — since reading an
 * already-computed `MeshRun[]` is cheap array indexing, not a fresh band
 * walk. The expensive part (`scan_all_column_runs`) is paid once per chunk
 * and reused across every LOD tier and every LOD transition after that, the
 * same amortization the old per-chunk surface cache had, without that
 * surface's single-height-per-column ceiling (see `accumulate_run`).
 */
export function bake_chunk_mesh_coarse(column_runs: readonly MeshRun[][], lod_step: LodStep): BakedChunkMesh {
  if (lod_step === 1) throw new Error("render: finest LOD requires per-run meshing");

  const res: u32 = CHUNK_CELL_SPAN / lod_step;
  const per_cell: ZBucketAccum[][] = new Array(res * res);
  let vertex_total: u32 = 0;
  let index_total: u32 = 0;

  for (let by: u32 = 0; by < res; by++) {
    for (let bx: u32 = 0; bx < res; bx++) {
      const buckets: ZBucketAccum[] = [];
      for (let j: u32 = 0; j < lod_step; j++) {
        const cy = by * lod_step + j;
        for (let k: u32 = 0; k < lod_step; k++) {
          const cx = bx * lod_step + k;
          for (const run of column_runs[cy * CHUNK_CELL_SPAN + cx]) accumulate_run(buckets, lod_step, run);
        }
      }
      per_cell[by * res + bx] = buckets;
      vertex_total += buckets.length * OPEN_BOTTOM_FACE_CNT * 4;
      index_total += buckets.length * OPEN_BOTTOM_FACE_CNT * 6;
    }
  }

  const builder = alloc_builder(vertex_total, index_total);
  for (let by: u32 = 0; by < res; by++) {
    for (let bx: u32 = 0; bx < res; bx++) {
      for (const acc of per_cell[by * res + bx]) emit_averaged_box(builder, bx, by, lod_step, acc);
    }
  }
  return finish_builder(builder);
}
