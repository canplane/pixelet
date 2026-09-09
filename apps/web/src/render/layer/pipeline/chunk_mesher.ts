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
import { Arena, Vector } from "@xpute/core/ds/arena.ts";
import type { Block } from "@xpute/core/ds/pool.ts";
import { MESH_POOL } from "./mesh_pool.ts";
import { CHUNK_CELL_SPAN, U_PER_CELL } from "@pxl/shared/bake/constants.ts";

import type { LodStep } from "@/render/lod.ts";
import { MaterialClass, NO_LAYER, SUB_CELL_NONE, unpack_sub_cell_fraction } from "./volume.ts";
import type { ColumnRunSet } from "./voxel_scan.ts";

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
  /**
   * The one pooled allocation every span below is a view into — including
   * chunk_bucket.ts's `visibility` (see carve_visibility). Its owner must
   * hand it back via release_chunk_mesh once no view can still be read; a
   * view outliving that release reads whatever the next borrower wrote.
   */
  block: Block;
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
  /**
   * vertex_cnt floats: this vertex's own box's face mask, the same six bits
   * run_face_mask produced. The shader reads it to shade only the edges that
   * border open air — an edge of the top face is a silhouette exactly when
   * the side face in that direction survived culling, so the question face
   * culling already answers is the same one edge shading needs.
   */
  faces: Float32Array;
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

/**
 * Dev-only, cumulative since page load.
 *
 * `total_runs`/`culled` are runs seen and runs with no visible face at all.
 * `faces_possible`/`faces_kept` are the per-face half (run_face_mask), and
 * they are the ones that answer "what did face culling actually buy on real
 * terrain" — the synthetic plateau in the tests says 45 faces where 125 were
 * emitted, but real chunks are not plateaus and the ratio is the number
 * worth watching, not the one that was hoped for.
 */
export const occlusion_stats = { total_runs: 0, culled: 0, faces_possible: 0, faces_kept: 0 };

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

// ---- Per-face visibility ----
//
// Bit i is FACES[i], so the order is +X, -X, +Y, -Y, +Z, -Z.
//
// Every box used to emit all five (or six) of its faces regardless of what
// sat next to it. On flat ground that is four hidden faces per box: a 64x64
// chunk of plateau emitted 4096 boxes x 5 faces = 81,920 vertices of which
// roughly four fifths could never be seen from anywhere, which is most of
// what made a dense urban chunk measure 780k vertices and 67MB.
//
// A run whose mask comes back empty is dropped entirely, which is what the
// separate whole-run occlusion test used to do — measured over half a
// million runs of real terrain that test fired zero times, because its
// condition is strictly narrower than this one.
const FACE_PX: u32 = 1 << 0;
const FACE_NX: u32 = 1 << 1;
const FACE_PY: u32 = 1 << 2;
const FACE_NY: u32 = 1 << 3;
const FACE_PZ: u32 = 1 << 4;
const FACE_NZ: u32 = 1 << 5;
const ALL_FACES: u32 = (1 << 6) - 1;
const OPEN_BOTTOM_FACES: u32 = ALL_FACES & ~FACE_NZ;

// ---- Edge shading bits ----
//
// One rule covers every edge of every face. For a face with outward normal
// `n`, its edge toward an in-plane direction `d` is a crease when either:
//
//   convex   the face toward `d` also exists — nothing is out that way, so
//            the edge is a silhouette. Already answered by the six bits
//            above, for free.
//   concave  the cell at `d + n` is solid — the surface turns inward there.
//
// Everything below exists to answer that second question for all twelve
// (face, edge) pairs. `d + n` is always one of three shapes: a lateral
// neighbour one cell up, one cell down, or a diagonal in the XY plane.
//
// The six face bits cannot substitute for any of it. A neighbour level with
// this run and one above it both hide the same side face, so flat ground and
// the foot of a riser clear the same bit; and a wall that runs straight on
// and a wall that turns inward both have their perpendicular bit cleared by
// the cell they share.
const RISES_PX: u32 = 1 << 6;
const RISES_NX: u32 = 1 << 7;
const RISES_PY: u32 = 1 << 8;
const RISES_NY: u32 = 1 << 9;
const DIPS_PX: u32 = 1 << 10;
const DIPS_NX: u32 = 1 << 11;
const DIPS_PY: u32 = 1 << 12;
const DIPS_NY: u32 = 1 << 13;

// The XY diagonals, for the vertical seam where a wall turns. Gated: a
// diagonal is only scanned where a wall could actually turn — one of the two
// faces meeting there exists while the other's axis neighbour hides it. Flat
// ground has no side faces and pays nothing.
const DIAG_PP: u32 = 1 << 14;
const DIAG_PN: u32 = 1 << 15;
const DIAG_NP: u32 = 1 << 16;
const DIAG_NN: u32 = 1 << 17;

// Whether a lateral edge is a convex crease, which is *not* the same as
// whether its face exists.
//
// The two answers diverge at a chunk's own boundary, where the neighbouring
// chunk's columns are not in this scan. Emitting the face there is the
// conservative choice and has to stay — culling one that should be visible
// is a hole. Shading its edge is the opposite: the conservative choice is to
// leave it alone, because a wrong line is drawn on every flat seam in the
// world, once every CHUNK_CELL_SPAN cells, while a wrong omission costs an
// outline only where a crease happens to run along a chunk edge.
//
// So the face bits keep deciding emission and these decide shading. The
// vertical edges need no equivalent: their neighbour is this column's own
// run list, which is always known.
const CREASE_PX: u32 = 1 << 18;
const CREASE_NX: u32 = 1 << 19;
const CREASE_PY: u32 = 1 << 20;
const CREASE_NY: u32 = 1 << 21;

// Whether the cell above / below this run is filled. Not the same question
// as "did FACE_PZ survive culling", which is what the shader used to read:
// a placed object is exempt from culling (it must not open a hole under
// itself — 2026-08-16) but it is still solid matter sitting there, so a
// wall capped by one is not a silhouette. Shading has to ask what is
// actually present; culling gets to be conservative about it.
const CAP_PZ: u32 = 1 << 22;
const CAP_NZ: u32 = 1 << 23;

function face_cnt_of(mask: u32): u32 {
  let n: u32 = 0;
  for (let bits = mask; bits !== 0; bits &= bits - 1) n++;
  return n;
}

// ---- Pooled block layout ----
//
// Every span of a baked mesh lives in one block, in this fixed order. They
// share a lifetime exactly (the whole mesh dies at once, when its owner
// does), so splitting them across eight allocations bought nothing and cost
// eight times the collector pressure — measured: a zoom-out that rebuilds
// ~2000 chunks moved +344MB in a single frame, 8.4s of it off-thread.
// `visibility` is reserved here but written by chunk_bucket.ts (see
// carve_visibility): the mesher owns the layout, never the role policy.
const F32_PER_VERTEX: u32 = 3 /* position */ + 3 /* normal */ + 3 /* local_pos */ + 3 /* box_scale */ + 3 /* rgb */ + 4 /* layer_taps */ + 1 /* faces */;
const VIS_F32_PER_VERTEX: u32 = 1;

function block_bytes(vertex_total: u32, index_total: u32): u32 {
  return (vertex_total * (F32_PER_VERTEX + VIS_F32_PER_VERTEX) + index_total) * 4;
}

/** Hands one baked mesh's block back. Every view into it is dead afterwards. */
export function release_chunk_mesh(mesh: BakedChunkMesh): void {
  MESH_POOL.release(mesh.block);
}

/**
 * The per-vertex LOD selector's own span inside an already-baked mesh's
 * block. Not part of BakedChunkMesh: this module bakes real geometry and
 * never a per-role default, so the span is reserved here and filled by
 * chunk_bucket.ts#with_visibility. A recycled block arrives dirty, so the
 * caller must write every element before the mesh is drawn.
 */
export function carve_visibility(mesh: BakedChunkMesh): Float32Array {
  const block = mesh.block;
  return new Float32Array(block.buf, block.byte_offset + mesh.vertex_cnt * F32_PER_VERTEX * 4, mesh.vertex_cnt * VIS_F32_PER_VERTEX);
}

/** Vertex/index writer over one pooled block — callers precompute the exact
 * vertex/index totals (every emit_box call's own face count is known ahead
 * of time from include_bottom alone), so this never resizes, and every
 * element of every span below is written before the mesh is used, which is
 * what makes a dirty recycled block safe. */
type MeshBuilder = {
  block: Block;
  position: Float32Array;
  normal: Float32Array;
  local_pos: Float32Array;
  box_scale: Float32Array;
  rgb: Float32Array;
  layer_taps: Float32Array;
  faces: Float32Array;
  index: Uint32Array;
  v_cnt: u32;
  i_cnt: u32;
};

function alloc_builder(vertex_total: u32, index_total: u32): MeshBuilder {
  const block = MESH_POOL.acquire(block_bytes(vertex_total, index_total));
  const buf = block.buf;
  // Every span is measured from the block's own start, never the buffer's:
  // a region is shared with other live chunks' blocks.
  let off = block.byte_offset;
  const span = (floats: u32): Float32Array => {
    const view = new Float32Array(buf, off, floats);
    off += floats * 4;
    return view;
  };
  return {
    block,
    position: span(vertex_total * 3),
    normal: span(vertex_total * 3),
    local_pos: span(vertex_total * 3),
    box_scale: span(vertex_total * 3),
    rgb: span(vertex_total * 3),
    layer_taps: span(vertex_total * 4),
    faces: span(vertex_total),
    // visibility's own span sits here, carved on demand by carve_visibility.
    index: new Uint32Array(buf, block.byte_offset + vertex_total * (F32_PER_VERTEX + VIS_F32_PER_VERTEX) * 4, index_total),
    v_cnt: 0,
    i_cnt: 0,
  };
}

function finish_builder(builder: MeshBuilder): BakedChunkMesh {
  return {
    block: builder.block,
    vertex_cnt: builder.v_cnt,
    position: builder.position,
    normal: builder.normal,
    local_pos: builder.local_pos,
    box_scale: builder.box_scale,
    rgb: builder.rgb,
    layer_taps: builder.layer_taps,
    faces: builder.faces,
    index: builder.index,
  };
}

/**
 * Bakes one box's own real vertices (5 faces, bottom dropped, or all 6)
 * directly into `builder`.
 *
 * Scalar parameters rather than the `[f64, f64, f64]` tuples the box/size
 * naturally come as: this is called once per surviving run, thousands of
 * times per chunk and ~2000 chunks per zoom-out, so a pair of throwaway
 * three-element arrays per call is itself a measurable share of the
 * collector pressure the pooled block above exists to remove.
 */
function emit_box(
  builder: MeshBuilder,
  min_x: f64,
  min_y: f64,
  min_z: f64,
  size_x: f64,
  size_y: f64,
  size_z: f64,
  r: f64,
  g: f64,
  b: f64,
  tap0: f64,
  tap1: f64,
  tap2: f64,
  tap3: f64,
  faces: u32,
): void {
  const cx = min_x + size_x * 0.5, cy = min_y + size_y * 0.5, cz = min_z + size_z * 0.5;

  for (let f: u32 = 0; f < FULL_FACE_CNT; f++) {
    if ((faces & (1 << f)) === 0) continue;
    const face = FACES[f];
    const base_v = builder.v_cnt;
    for (let c = 0; c < 4; c++) {
      const corner = face.corners[c];
      const vi = builder.v_cnt++;
      const po = vi * 3, lo = vi * 4;
      builder.position[po] = cx + corner[0] * size_x;
      builder.position[po + 1] = cy + corner[1] * size_y;
      builder.position[po + 2] = cz + corner[2] * size_z;
      builder.normal[po] = face.normal[0];
      builder.normal[po + 1] = face.normal[1];
      builder.normal[po + 2] = face.normal[2];
      builder.local_pos[po] = corner[0];
      builder.local_pos[po + 1] = corner[1];
      builder.local_pos[po + 2] = corner[2];
      builder.box_scale[po] = size_x;
      builder.box_scale[po + 1] = size_y;
      builder.box_scale[po + 2] = size_z;
      builder.rgb[po] = r;
      builder.rgb[po + 1] = g;
      builder.rgb[po + 2] = b;
      builder.layer_taps[lo] = tap0;
      builder.layer_taps[lo + 1] = tap1;
      builder.layer_taps[lo + 2] = tap2;
      builder.layer_taps[lo + 3] = tap3;
      builder.faces[vi] = faces;
    }
    builder.index[builder.i_cnt++] = base_v;
    builder.index[builder.i_cnt++] = base_v + 1;
    builder.index[builder.i_cnt++] = base_v + 2;
    builder.index[builder.i_cnt++] = base_v;
    builder.index[builder.i_cnt++] = base_v + 2;
    builder.index[builder.i_cnt++] = base_v + 3;
  }
}

/**
 * Whether a column has a run spanning [z0, z1] that would hide a face
 * against it. An OBJECT-class run never counts as covering — the same
 * exclusion the retired is_run_occluded made through is_topmost_run, for the
 * same reason and with the same bug behind it (2026-08-16: placed objects
 * visibly dug into the terrain beneath them). A standalone placed object
 * does not seal its neighbour's face the way a continuous layer of terrain
 * or structure does, because the camera is near-top-down but still oblique,
 * so a face it "hides" is one you can see past.
 */
function column_spans_range(set: ColumnRunSet, col: u32, z0: i32, z1: i32, skip_objects: boolean): boolean {
  const end = set.start[col + 1];
  for (let i = set.start[col]; i < end; i++) {
    // merge_column_runs emits a column ascending, so once a run starts above
    // the range no later one can contain it. Without this the scan is
    // O(runs) every time and run_face_mask calls it six times per run —
    // millions of comparisons on a dense chunk, which showed up directly as
    // commit_ms p95 going 10.9ms -> 17.9ms when face culling landed.
    if (set.z0[i] > z0) return false;
    if (set.sub_sz[i] !== SUB_CELL_NONE) continue;
    if (skip_objects && set.material_class[i] === MaterialClass.OBJECT) continue;
    if (set.z1[i] >= z1) return true;
  }
  return false;
}

/** Enough to drop a face: a placed object does not count, per this
 * function's own doc comment above. */
function column_covers_range_opaque(set: ColumnRunSet, col: u32, z0: i32, z1: i32): boolean {
  return column_spans_range(set, col, z0, z1, true);
}

/** Enough to stop an edge being a silhouette: anything solid counts,
 * including a placed object. Nothing is culled off this answer, so the
 * caution that keeps objects out of the one above does not apply. */
function column_fills_range(set: ColumnRunSet, col: u32, z0: i32, z1: i32): boolean {
  return column_spans_range(set, col, z0, z1, false);
}

const HIDES_SIDE: u32 = 1;
const RISES_ABOVE: u32 = 2;
const EXTENDS_BELOW: u32 = 4;
/** Same span as HIDES_SIDE, but counting placed objects too. The face still
 * survives beside one; the edge just stops being a silhouette. */
const ABUTS_SIDE: u32 = 8;

/**
 * Both questions a neighbour column can answer about one run, in one pass.
 *
 * Whether it hides my side face (spans my whole range), whether anything is
 * flush against that side at all, whether it occupies the cell over my top,
 * and whether it occupies the cell under my bottom are four separate facts
 * that one ascending walk settles together. Asking them as four calls would
 * quadruple the neighbour scans, and run_face_mask already makes four.
 *
 * Only HIDES_SIDE excludes placed objects, because only HIDES_SIDE drops a
 * face. The other three feed edge shading, where treating an object as
 * absent is what draws a bevel down the join between a user cube and the
 * terrain it sits flush against.
 */
function neighbour_relation(set: ColumnRunSet, col: u32, z0: i32, z1: i32): u32 {
  const above = z1 + 1;
  const below = z0 - 1;
  const all = HIDES_SIDE | ABUTS_SIDE | RISES_ABOVE | EXTENDS_BELOW;
  let rel: u32 = 0;
  const end = set.start[col + 1];
  for (let i = set.start[col]; i < end; i++) {
    const r_z0 = set.z0[i];
    if (r_z0 > above) break;
    if (set.sub_sz[i] !== SUB_CELL_NONE) continue;
    const r_z1 = set.z1[i];
    const spans = r_z0 <= z0 && r_z1 >= z1;
    if (spans) rel |= ABUTS_SIDE;
    if (spans && set.material_class[i] !== MaterialClass.OBJECT) rel |= HIDES_SIDE;
    if (r_z0 <= above && r_z1 >= above) rel |= RISES_ABOVE;
    if (r_z0 <= below && r_z1 >= below) rel |= EXTENDS_BELOW;
    if (rel === all) break;
  }
  return rel;
}

/* is_run_occluded lived here, with is_topmost_run and column_covers_range
 * behind it: a run buried on all four sides *and* covered from above was
 * dropped whole. Measured over half a million runs of real terrain it fired
 * exactly zero times, while costing four coverage scans on every one of
 * them, because the condition it tested is strictly narrower than the one
 * run_face_mask already tests. A run with no visible face is a run whose
 * mask comes back empty, and that includes cases the whole-run test refused
 * — it excluded chunk-edge columns, which the mask handles by keeping the
 * edge face rather than by keeping the whole run.
 *
 * occlusion_stats.culled now counts empty masks, which is the same quantity
 * the name always meant. */

function run_include_bottom(set: ColumnRunSet, i: u32): boolean {
  return set.material_class[i] === MaterialClass.OBJECT;
}

/**
 * Which of a cube run's own six faces can be seen at all.
 *
 * A face is dropped only when a neighbouring column has a solid,
 * non-sub-cell run spanning this run's *entire* z range — whole-face
 * granularity, never split. A partially-covered face stays whole, which
 * wastes a few vertices and can never open a hole; splitting it into the
 * uncovered sub-intervals is what greedy meshing would do next.
 *
 * Chunk-edge columns never cull sideways, same reasoning as
 * is_run_occluded: the neighbouring chunk's data is not available here and
 * assuming it solid is a real correctness risk for a cosmetic win.
 *
 * A sub-cell run keeps every face: it fills a fraction of its own voxel, so
 * a neighbour filling all of its own still does not cover this one's face.
 */
function run_face_mask(set: ColumnRunSet, cx: u32, cy: u32, i: u32): u32 {
  const mask_all = run_include_bottom(set, i) ? ALL_FACES : OPEN_BOTTOM_FACES;
  if (set.sub_sz[i] !== SUB_CELL_NONE) return mask_all;

  let mask = mask_all;
  const col = cy * CHUNK_CELL_SPAN + cx;
  const z0 = set.z0[i], z1 = set.z1[i];
  if (column_covers_range_opaque(set, col, z1 + 1, z1 + 1)) mask &= ~FACE_PZ;
  if ((mask & FACE_NZ) !== 0 && column_covers_range_opaque(set, col, z0 - 1, z0 - 1)) mask &= ~FACE_NZ;
  // Asked of everything solid, not only of what would have culled the face:
  // a wall capped by a user cube is capped, whether or not the cube was
  // allowed to drop the face under it.
  if (column_fills_range(set, col, z1 + 1, z1 + 1)) mask |= CAP_PZ;
  if (column_fills_range(set, col, z0 - 1, z0 - 1)) mask |= CAP_NZ;

  // A chunk-edge column keeps its side face and gets no crease, concave or
  // convex: the neighbouring chunk's columns are not in this scan.
  if (cx + 1 < CHUNK_CELL_SPAN) {
    const rel = neighbour_relation(set, col + 1, z0, z1);
    if ((rel & HIDES_SIDE) !== 0) mask &= ~FACE_PX;
    if ((rel & ABUTS_SIDE) === 0) mask |= CREASE_PX;
    if ((rel & RISES_ABOVE) !== 0) mask |= RISES_PX;
    if ((rel & EXTENDS_BELOW) !== 0) mask |= DIPS_PX;
  }
  if (cx > 0) {
    const rel = neighbour_relation(set, col - 1, z0, z1);
    if ((rel & HIDES_SIDE) !== 0) mask &= ~FACE_NX;
    if ((rel & ABUTS_SIDE) === 0) mask |= CREASE_NX;
    if ((rel & RISES_ABOVE) !== 0) mask |= RISES_NX;
    if ((rel & EXTENDS_BELOW) !== 0) mask |= DIPS_NX;
  }
  if (cy + 1 < CHUNK_CELL_SPAN) {
    const rel = neighbour_relation(set, col + CHUNK_CELL_SPAN, z0, z1);
    if ((rel & HIDES_SIDE) !== 0) mask &= ~FACE_PY;
    if ((rel & ABUTS_SIDE) === 0) mask |= CREASE_PY;
    if ((rel & RISES_ABOVE) !== 0) mask |= RISES_PY;
    if ((rel & EXTENDS_BELOW) !== 0) mask |= DIPS_PY;
  }
  if (cy > 0) {
    const rel = neighbour_relation(set, col - CHUNK_CELL_SPAN, z0, z1);
    if ((rel & HIDES_SIDE) !== 0) mask &= ~FACE_NY;
    if ((rel & ABUTS_SIDE) === 0) mask |= CREASE_NY;
    if ((rel & RISES_ABOVE) !== 0) mask |= RISES_NY;
    if ((rel & EXTENDS_BELOW) !== 0) mask |= DIPS_NY;
  }

  // One diagonal per quadrant, and only where a wall could actually turn:
  // one of the two faces meeting there exists while the other's own axis
  // neighbour hides it.
  const wants_diagonal = (a: u32, b: u32) => ((mask & a) !== 0 && (mask & b) === 0) || ((mask & b) !== 0 && (mask & a) === 0);
  if (cx + 1 < CHUNK_CELL_SPAN && cy + 1 < CHUNK_CELL_SPAN && wants_diagonal(FACE_PX, FACE_PY)) {
    if (column_fills_range(set, col + CHUNK_CELL_SPAN + 1, z0, z1)) mask |= DIAG_PP;
  }
  if (cx + 1 < CHUNK_CELL_SPAN && cy > 0 && wants_diagonal(FACE_PX, FACE_NY)) {
    if (column_fills_range(set, col - CHUNK_CELL_SPAN + 1, z0, z1)) mask |= DIAG_PN;
  }
  if (cx > 0 && cy + 1 < CHUNK_CELL_SPAN && wants_diagonal(FACE_NX, FACE_PY)) {
    if (column_fills_range(set, col + CHUNK_CELL_SPAN - 1, z0, z1)) mask |= DIAG_NP;
  }
  if (cx > 0 && cy > 0 && wants_diagonal(FACE_NX, FACE_NY)) {
    if (column_fills_range(set, col - CHUNK_CELL_SPAN - 1, z0, z1)) mask |= DIAG_NN;
  }
  return mask;
}

/**
 * Pure and allocation-owning: each call returns one exactly-sized immutable
 * baked mesh for one 64x64 chunk and one LOD step, from a chunk's
 * `column_runs` (see `voxel_scan.ts#scan_all_column_runs`) — scanned once
 * from the live `ChunkVolume` and reusable across every LOD rebuild after
 * that, long after the volume itself is released back to its pool.
 */
export function bake_chunk_mesh(set: ColumnRunSet, lod_step: LodStep): BakedChunkMesh {
  if (lod_step === 1) return bake_chunk_mesh_fine(set);
  return bake_chunk_mesh_coarse(set, lod_step);
}

/**
 * Which runs survived occlusion, as (cx, cy, run index, face mask) quads
 * rather than one object per entry: a dense chunk survives thousands of
 * runs, and the second pass only needs to find each run again, not to hold
 * it. The mask is carried rather than recomputed because the counting pass
 * has to know the visible face count exactly — the block below is sized from
 * it — and the neighbour lookups that produce it are the expensive part.
 * Module-level and rewound by hand: bake_chunk_mesh_fine is synchronous and
 * non-reentrant, so one scratch store serves every chunk.
 */
const SURVIVING = new Vector(Uint32Array, { init_cap: 4 * (1 << 12) });

function bake_chunk_mesh_fine(set: ColumnRunSet): BakedChunkMesh {
  const base = SURVIVING.len;
  let vertex_total: u32 = 0;
  let index_total: u32 = 0;

  for (let cy: u32 = 0; cy < CHUNK_CELL_SPAN; cy++) {
    for (let cx: u32 = 0; cx < CHUNK_CELL_SPAN; cx++) {
      const col = cy * CHUNK_CELL_SPAN + cx;
      const end = set.start[col + 1];
      for (let i = set.start[col]; i < end; i++) {
        occlusion_stats.total_runs++;
        const mask = run_face_mask(set, cx, cy, i);
        occlusion_stats.faces_possible += run_include_bottom(set, i) ? FULL_FACE_CNT : OPEN_BOTTOM_FACE_CNT;
        // Concave bits shade an edge but never keep a run alive on their own.
        if ((mask & ALL_FACES) === 0) {
          occlusion_stats.culled++;
          continue;
        }
        SURVIVING.push(cx);
        SURVIVING.push(cy);
        SURVIVING.push(i);
        SURVIVING.push(mask);
        const faces = face_cnt_of(mask & ALL_FACES);
        occlusion_stats.faces_kept += faces;
        vertex_total += faces * 4;
        index_total += faces * 6;
      }
    }
  }

  const builder = alloc_builder(vertex_total, index_total);
  for (let s_i = base; s_i < SURVIVING.len; s_i += 4) {
    const cx = SURVIVING.get(s_i), cy = SURVIVING.get(s_i + 1), i = SURVIVING.get(s_i + 2);
    // run_to_box's own math, inlined against the packed spans: it returns a
    // {min, max} pair of arrays, which is three allocations per surviving
    // run and the reason this path does not call it. voxel_scan.ts keeps it
    // for the raycast, where it runs once per hover.
    const sub_sz = set.sub_sz[i];
    let min_x: f64, min_y: f64, min_z: f64, size_x: f64, size_y: f64, size_z: f64;
    if (sub_sz === SUB_CELL_NONE) {
      min_x = cx * U_PER_CELL;
      min_y = cy * U_PER_CELL;
      min_z = set.z0[i] * U_PER_CELL;
      size_x = U_PER_CELL;
      size_y = U_PER_CELL;
      size_z = (set.z1[i] + 1 - set.z0[i]) * U_PER_CELL;
    } else {
      const sz = unpack_sub_cell_fraction(sub_sz) * U_PER_CELL;
      min_x = (cx + unpack_sub_cell_fraction(set.sub_ox[i])) * U_PER_CELL;
      min_y = (cy + unpack_sub_cell_fraction(set.sub_oy[i])) * U_PER_CELL;
      min_z = (set.z0[i] + unpack_sub_cell_fraction(set.sub_oz[i])) * U_PER_CELL;
      size_x = sz;
      size_y = sz;
      size_z = sz;
    }
    const layer = set.layer_idx[i];
    emit_box(
      builder,
      min_x,
      min_y,
      min_z,
      size_x,
      size_y,
      size_z,
      srgb_to_linear(set.r[i] / 255),
      srgb_to_linear(set.g[i] / 255),
      srgb_to_linear(set.b[i] / 255),
      layer,
      layer,
      layer,
      layer,
      SURVIVING.get(s_i + 3),
    );
  }
  SURVIVING.truncate(base);
  return finish_builder(builder);
}

/** tap0..tap3: up to 4 distinct texture-array layers touching this bucket,
 * first-encountered order (NO_LAYER — objects, always — never added). Four
 * fields rather than an array because these live in a reused arena slot: a
 * nested array per accumulator would reintroduce exactly the per-cell
 * garbage the arena exists to remove, and the cap is a fixed vec4 anyway. */
type ZBucketAccum = { bz: i32; pr: f64; pg: f64; pb: f64; coverage: f64; tap0: u32; tap1: u32; tap2: u32; tap3: u32; tap_cnt: u32 };

/**
 * Every coarse cell's Z buckets, bump-allocated and rewound per chunk.
 * `bake_chunk_mesh_coarse` fills one cell completely before starting the
 * next, so a cell's buckets are contiguous here and CELL_RANGE below only
 * has to remember (start, count) — no per-cell container at all, where the
 * previous shape allocated `res^2` arrays plus an object per bucket on every
 * single rebuild.
 */
const BUCKETS = new Arena<ZBucketAccum>(
  () => ({ bz: 0, pr: 0, pg: 0, pb: 0, coverage: 0, tap0: NO_LAYER, tap1: NO_LAYER, tap2: NO_LAYER, tap3: NO_LAYER, tap_cnt: 0 }),
  { init_cap: 1 << 12 },
);
const CELL_RANGE = new Vector(Uint32Array, { init_cap: 2 * (1 << 10) });

/**
 * Linear scan, not a Map: a coarse cell's real Z-bucket count is small
 * (bounded by how many bands actually stack under it), and scanning the
 * cell's own contiguous arena range avoids a hash-table container per cell
 * — `bake_chunk_mesh_coarse` calls this up to `res^2` times per build
 * (1024x for `lod_step=2`).
 *
 * Returns the arena index. A returned index at or past `start + cnt` is a
 * freshly appended bucket, which is how the caller knows to widen its count
 * (append-at-the-end is the only growth, since one cell is filled at a time).
 */
function find_or_create_bucket(start: u32, cnt: u32, bz: i32): u32 {
  for (let i = start; i < start + cnt; i++) {
    if (BUCKETS.get(i).bz === bz) return i;
  }
  const acc = BUCKETS.alloc();
  acc.bz = bz;
  acc.pr = 0;
  acc.pg = 0;
  acc.pb = 0;
  acc.coverage = 0;
  acc.tap_cnt = 0;
  return BUCKETS.len - 1;
}

function tap_at(acc: ZBucketAccum, i: u32): u32 {
  return i === 0 ? acc.tap0 : i === 1 ? acc.tap1 : i === 2 ? acc.tap2 : acc.tap3;
}

/** Same small-N linear-scan reasoning as find_or_create_bucket — a bucket
 * realistically sees a handful of distinct layers, not enough to justify a
 * hash table. Caps at 4 (matches layer_taps' fixed-size vec4 slot); beyond
 * that, whichever 4 were encountered first win. */
function collect_layer_tap(acc: ZBucketAccum, layer_idx: u32): void {
  if (layer_idx === NO_LAYER || acc.tap_cnt >= 4) return;
  for (let i: u32 = 0; i < acc.tap_cnt; i++) {
    if (tap_at(acc, i) === layer_idx) return;
  }
  if (acc.tap_cnt === 0) acc.tap0 = layer_idx;
  else if (acc.tap_cnt === 1) acc.tap1 = layer_idx;
  else if (acc.tap_cnt === 2) acc.tap2 = layer_idx;
  else acc.tap3 = layer_idx;
  acc.tap_cnt++;
}

/**
 * Folds one fine-column run into whichever `lod_step`-tall Z buckets it
 * overlaps. Material class is intentionally ignored here — terrain and
 * objects are summed into the same average, on purpose (a coarse cell's
 * color is whatever's really in it, air included via `coverage` staying
 * below `lod_step^3`). A sub-cell run occupies one fine voxel fractionally;
 * its weight is the volumetric fraction (linear fraction cubed), not 1.
 *
 * Takes and returns the cell's own (start, count) range into BUCKETS — a
 * run can create a bucket the cell did not have yet, and the count is the
 * only thing that grows.
 */
function accumulate_run(start: u32, cnt: u32, lod_step: LodStep, set: ColumnRunSet, i: u32): u32 {
  const run_z0 = set.z0[i], run_z1 = set.z1[i];
  const run_r = set.r[i], run_g = set.g[i], run_b = set.b[i], run_layer = set.layer_idx[i];
  if (set.sub_sz[i] !== SUB_CELL_NONE) {
    const frac = unpack_sub_cell_fraction(set.sub_sz[i]);
    const w = frac * frac * frac;
    if (w <= 0) return cnt;
    const idx = find_or_create_bucket(start, cnt, Math.floor(run_z0 / lod_step));
    const acc = BUCKETS.get(idx);
    acc.pr += run_r * w;
    acc.pg += run_g * w;
    acc.pb += run_b * w;
    acc.coverage += w;
    collect_layer_tap(acc, run_layer);
    return Math.max(cnt, idx - start + 1);
  }

  const bz_lo: i32 = Math.floor(run_z0 / lod_step);
  const bz_hi: i32 = Math.floor(run_z1 / lod_step);
  for (let bz = bz_lo; bz <= bz_hi; bz++) {
    const bucket_lo = bz * lod_step;
    const bucket_hi = bucket_lo + lod_step - 1;
    const overlap = Math.min(run_z1, bucket_hi) - Math.max(run_z0, bucket_lo) + 1;
    if (overlap <= 0) continue;
    const idx = find_or_create_bucket(start, cnt, bz);
    cnt = Math.max(cnt, idx - start + 1);
    const acc = BUCKETS.get(idx);
    acc.pr += run_r * overlap;
    acc.pg += run_g * overlap;
    acc.pb += run_b * overlap;
    acc.coverage += overlap;
    collect_layer_tap(acc, run_layer);
  }
  return cnt;
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

  // The shader always averages 4 taps at 25% each — padding unused slots
  // with NO_LAYER (falls back to the flat instance color) dilutes toward
  // flat color whenever fewer than 4 distinct layers touched this bucket,
  // which is the common case (most buckets are one material, not four).
  // Cycling the real taps into every slot instead keeps the blend 100%
  // texture-derived whenever at least one real layer exists, matching what
  // LOD1 already does (all 4 of its taps are the same one real layer_idx).
  const n = acc.tap_cnt;
  emit_box(
    builder,
    (bx + 0.5) * span - half,
    (by + 0.5) * span - half,
    cz - half,
    sz,
    sz,
    sz,
    srgb_to_linear(acc.pr / acc.coverage / 255),
    srgb_to_linear(acc.pg / acc.coverage / 255),
    srgb_to_linear(acc.pb / acc.coverage / 255),
    n > 0 ? tap_at(acc, 0 % n) : NO_LAYER,
    n > 0 ? tap_at(acc, 1 % n) : NO_LAYER,
    n > 0 ? tap_at(acc, 2 % n) : NO_LAYER,
    n > 0 ? tap_at(acc, 3 % n) : NO_LAYER,
    // No neighbour culling at coarse LOD: emit_averaged_box shrinks a box by
    // its fill fraction, so adjacent coarse boxes generally do not touch and
    // a hidden-face test would be both wrong and useless.
    OPEN_BOTTOM_FACES,
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
 * already-packed run set is cheap array indexing, not a fresh band
 * walk. The expensive part (`scan_all_column_runs`) is paid once per chunk
 * and reused across every LOD tier and every LOD transition after that, the
 * same amortization the old per-chunk surface cache had, without that
 * surface's single-height-per-column ceiling (see `accumulate_run`).
 */
export function bake_chunk_mesh_coarse(set: ColumnRunSet, lod_step: LodStep): BakedChunkMesh {
  if (lod_step === 1) throw new Error("render: finest LOD requires per-run meshing");

  const res: u32 = CHUNK_CELL_SPAN / lod_step;
  const bucket_base = BUCKETS.len;
  const range_base = CELL_RANGE.len;
  let vertex_total: u32 = 0;
  let index_total: u32 = 0;

  for (let by: u32 = 0; by < res; by++) {
    for (let bx: u32 = 0; bx < res; bx++) {
      const start = BUCKETS.len;
      let cnt: u32 = 0;
      for (let j: u32 = 0; j < lod_step; j++) {
        const cy = by * lod_step + j;
        for (let k: u32 = 0; k < lod_step; k++) {
          const cx = bx * lod_step + k;
          const col = cy * CHUNK_CELL_SPAN + cx;
          const end = set.start[col + 1];
          for (let i = set.start[col]; i < end; i++) cnt = accumulate_run(start, cnt, lod_step, set, i);
        }
      }
      CELL_RANGE.push(start);
      CELL_RANGE.push(cnt);
      vertex_total += cnt * OPEN_BOTTOM_FACE_CNT * 4;
      index_total += cnt * OPEN_BOTTOM_FACE_CNT * 6;
    }
  }

  const builder = alloc_builder(vertex_total, index_total);
  for (let by: u32 = 0; by < res; by++) {
    for (let bx: u32 = 0; bx < res; bx++) {
      const ri = range_base + (by * res + bx) * 2;
      const start = CELL_RANGE.get(ri), cnt = CELL_RANGE.get(ri + 1);
      for (let i: u32 = 0; i < cnt; i++) emit_averaged_box(builder, bx, by, lod_step, BUCKETS.get(start + i));
    }
  }
  CELL_RANGE.truncate(range_base);
  BUCKETS.truncate(bucket_base);
  return finish_builder(builder);
}
