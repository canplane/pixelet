// @pxl/web/render/layer/pipeline/chunk_mesher.ts

/**
 * Pure 64x64 chunk -> baked GPU mesh.
 *
 * Bakes real per-vertex geometry directly, one merged mesh per chunk (cube
 * terrain AND cube objects together — never instanced, never split into two
 * meshes): every surviving run contributes its own box's actual vertices
 * into one growing set of typed arrays, the same "each block bakes its own
 * geometry into one shared per-chunk buffer" model Minecraft itself uses,
 * chosen specifically because it's geometry-agnostic — decor_layer.ts's own
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
import type { Block } from "@xpute/core/ds/buddy.ts";
import { CORE_ALLOC, CORE_POOL } from "./core_pool.ts";
import { CHUNK_CELL_SPAN, U_PER_CELL } from "@pxl/shared/bake/constants.ts";

import type { LodStep } from "@/render/lod.ts";
import { MaterialClass, NO_LAYER, pack_sub_cell_fraction, SUB_CELL_NONE, unpack_sub_cell_fraction, unpack_sub_cell_size } from "./volume.ts";
import { alloc_column_run_set, type ColumnRunSet, MAX_UNIT_CHUNKS, release_column_runs } from "./voxel_scan.ts";

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
 * Dev-only, cumulative since page load: how many coarse-LOD boxes this
 * session has written, and how many were dropped for being too diffuse to
 * draw. Exposed the same no-console way, as a line of the ledger report
 * (sched/log.ts).
 *
 * Nothing is actually dropped: a sparse cell shrinks instead, all the way
 * down. `low_fill` prices what a threshold-cull policy would have skipped,
 * so the question stays answerable with a count rather than a guess.
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
const ALL_CREASES: u32 = CREASE_PX | CREASE_NX | CREASE_PY | CREASE_NY;

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
  CORE_POOL.release(mesh.block);
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
  const block = CORE_POOL.acquire(block_bytes(vertex_total, index_total));
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
  // Shading asks whether the neighbour covers this run's side at all, so
  // consecutive runs count together: carving a cell and refilling it with
  // another material splits that column into terrain below and an object on
  // top, and no single run then spans a taller neighbour's range. Testing
  // one run at a time reported flat ground beside such a cell as a
  // silhouette and drew an outline around it.
  let covered_to = z0 - 1;
  const end = set.start[col + 1];
  for (let i = set.start[col]; i < end; i++) {
    const r_z0 = set.z0[i];
    if (r_z0 > above) break;
    if (set.sub_sz[i] !== SUB_CELL_NONE) continue;
    const r_z1 = set.z1[i];
    if (r_z0 <= covered_to + 1 && r_z1 > covered_to) covered_to = r_z1;
    if (covered_to >= z1) rel |= ABUTS_SIDE;
    // Culling stays single-run: it may be conservative, since a face kept
    // that could have gone costs vertices while one dropped that should
    // have stayed opens a hole.
    if (r_z0 <= z0 && r_z1 >= z1 && set.material_class[i] !== MaterialClass.OBJECT) rel |= HIDES_SIDE;
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
  // A shrunken coarse box floats and so technically has a visible underside,
  // but the camera looks down at this world and never gets under one. Not
  // worth a sixth face on every sparse cell in the distance.
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
 * Every lateral edge of such a box is a silhouette against whatever is
 * behind it, so it gets all four creases outright rather than by asking
 * neighbours who cannot answer for a box that does not touch them.
 */
function run_face_mask(set: ColumnRunSet, span: u32, cx: u32, cy: u32, i: u32): u32 {
  const mask_all = run_include_bottom(set, i) ? ALL_FACES : OPEN_BOTTOM_FACES;
  if (set.sub_sz[i] !== SUB_CELL_NONE) return mask_all | ALL_CREASES;

  let mask = mask_all;
  const col = cy * span + cx;
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
  if (cx + 1 < span) {
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
  if (cy + 1 < span) {
    const rel = neighbour_relation(set, col + span, z0, z1);
    if ((rel & HIDES_SIDE) !== 0) mask &= ~FACE_PY;
    if ((rel & ABUTS_SIDE) === 0) mask |= CREASE_PY;
    if ((rel & RISES_ABOVE) !== 0) mask |= RISES_PY;
    if ((rel & EXTENDS_BELOW) !== 0) mask |= DIPS_PY;
  }
  if (cy > 0) {
    const rel = neighbour_relation(set, col - span, z0, z1);
    if ((rel & HIDES_SIDE) !== 0) mask &= ~FACE_NY;
    if ((rel & ABUTS_SIDE) === 0) mask |= CREASE_NY;
    if ((rel & RISES_ABOVE) !== 0) mask |= RISES_NY;
    if ((rel & EXTENDS_BELOW) !== 0) mask |= DIPS_NY;
  }

  // One diagonal per quadrant, and only where a wall could actually turn:
  // one of the two faces meeting there exists while the other's own axis
  // neighbour hides it.
  const wants_diagonal = (a: u32, b: u32) => ((mask & a) !== 0 && (mask & b) === 0) || ((mask & b) !== 0 && (mask & a) === 0);
  if (cx + 1 < span && cy + 1 < span && wants_diagonal(FACE_PX, FACE_PY)) {
    if (column_fills_range(set, col + span + 1, z0, z1)) mask |= DIAG_PP;
  }
  if (cx + 1 < span && cy > 0 && wants_diagonal(FACE_PX, FACE_NY)) {
    if (column_fills_range(set, col - span + 1, z0, z1)) mask |= DIAG_PN;
  }
  if (cx > 0 && cy + 1 < span && wants_diagonal(FACE_NX, FACE_PY)) {
    if (column_fills_range(set, col + span - 1, z0, z1)) mask |= DIAG_NP;
  }
  if (cx > 0 && cy > 0 && wants_diagonal(FACE_NX, FACE_NY)) {
    if (column_fills_range(set, col - span - 1, z0, z1)) mask |= DIAG_NN;
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
  const cursor = begin_bake(set, lod_step);
  if (!cursor) throw new Error("render: bake_chunk_mesh while a stepped bake is open");
  while (!step_bake(cursor)) { /* run to completion */ }
  return finish_bake(cursor);
}

// ---- stepped bake ----
//
// The bake is a sequence of passes over module-level scratch, each of which
// walks rows of columns or a list of boxes in order and depends only on the
// passes before it having finished. That makes each pass resumable at a row
// or box boundary with no stitching: stop anywhere, resume, and the scratch
// holds exactly what an uninterrupted run would have left there.
//
//   fine    mask rows -> merge rows -> size + allocate -> emit boxes
//   coarse  accumulate rows -> pack rows -> (a fine bake over the result)
//
// One bake at a time, because the scratch is shared — `begin_bake` refuses a
// second while one is open, and the holder is expected to finish or abort
// it. Abort returns what the bake holds: the builder's pooled block once
// emit has started, and a coarse bake's intermediate run set.

/** Rows a mask or merge step walks, and boxes an emit step writes. Sized so
 * a step on a dense chunk stays well under a millisecond — the whole bake
 * was measured at 3.5 to 5.6 ms on average. */
const BAKE_ROWS_PER_STEP: u32 = 8;
const BAKE_BOXES_PER_STEP: u32 = 512;

export type BakeCursor = {
  fine: FineBake | null;
  coarse: CoarseBake | null;
  done: boolean;
};

let open_bake: BakeCursor | null = null;

/** Whether a bake is open — so a would-be holder can skip preparing one. */
export function bake_open(): boolean {
  return open_bake !== null;
}

/** Opens a bake of `set` at `lod_step`, or returns null while another is open. */
export function begin_bake(set: ColumnRunSet, lod_step: LodStep): BakeCursor | null {
  if (open_bake !== null) return null;
  open_bake = lod_step === 1 ? { fine: begin_fine(set, set.span, 1), coarse: null, done: false } : { fine: null, coarse: begin_coarse(set, lod_step), done: false };
  return open_bake;
}

/** One step. Returns true once the bake is complete, at which point
 * `finish_bake` hands the mesh over. */
export function step_bake(cursor: BakeCursor): boolean {
  if (!cursor.done) cursor.done = cursor.coarse ? step_coarse(cursor.coarse) : step_fine(cursor.fine!);
  return cursor.done;
}

export function finish_bake(cursor: BakeCursor): BakedChunkMesh {
  if (cursor !== open_bake || !cursor.done) {
    throw new Error("render: finish_bake on a bake that is not the open, complete one");
  }
  const mesh = cursor.coarse ? finish_coarse(cursor.coarse) : finish_fine(cursor.fine!);
  open_bake = null;
  return mesh;
}

/** Drops a bake without finishing it, returning everything it holds. */
export function abort_bake(cursor: BakeCursor): void {
  if (cursor !== open_bake) return;
  if (cursor.coarse) abort_coarse(cursor.coarse);
  else abort_fine(cursor.fine!);
  open_bake = null;
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
const SURVIVING = new Vector(Uint32Array, { init_cap: 6 * (1 << 12), alloc: CORE_ALLOC });

/**
 * Greedy merge over the surviving runs, in the run domain rather than the
 * face domain.
 *
 * Two adjacent columns holding the same run — identical z span, identical
 * material — are one box two cells wide, and emit_box already takes a size:
 * the coarse path has always emitted boxes spanning many cells, and
 * `a_box_scale` carries the real extent so UV tiling and crease distance
 * both follow without a shader change. So merging costs no new emit path.
 *
 * Face-level greedy meshing would merge more (it can join tops whose runs
 * sit on different substrate), but it needs a separate quad emitter, its own
 * per-direction plane, and its own crease bookkeeping. This gets the case
 * that dominates real terrain — flat ground on a continuous base — for a
 * fraction of the surface area.
 *
 * The merge condition is stricter than "same material" in two ways that both
 * matter. The shared boundary must not be a crease, which identical z spans
 * already guarantee (each column covers the other, so both side faces are
 * culled), and it must be checked rather than assumed because concave and
 * diagonal bits can be set where no face survives. And the bits on the two
 * edges *perpendicular* to the merge have to agree across every cell, or the
 * merged box would claim one crease state for a border where the cells
 * disagreed.
 */
/** The bits that make one cell's +X border a seam: a face to draw there, or
 * an edge to shade. A merge may not swallow a border carrying any of them. */
const X_SEAM_BITS = FACE_PX | CREASE_PX | RISES_PX | DIPS_PX;
const Y_SEAM_BITS = FACE_PY | CREASE_PY | RISES_PY | DIPS_PY;

/** Bits that describe the +X/-X borders, which a merge along X consumes. */
const X_BORDER_BITS = FACE_PX | FACE_NX | CREASE_PX | CREASE_NX | RISES_PX | RISES_NX | DIPS_PX | DIPS_NX |
  DIAG_PP | DIAG_PN | DIAG_NP | DIAG_NN;
/** Same for Y. The diagonals belong to both, so a merge on either axis
 * requires them equal — a merged box has one diagonal state per corner. */
const Y_BORDER_BITS = FACE_PY | FACE_NY | CREASE_PY | CREASE_NY | RISES_PY | RISES_NY | DIPS_PY | DIPS_NY |
  DIAG_PP | DIAG_PN | DIAG_NP | DIAG_NN;

/**
 * The mask a merged rectangle carries.
 *
 * Not the near corner's — a merge along X consumes the border at *both*
 * ends, so the +X bits belong to the far column and only the -X ones are
 * this cell's. Same for Y, and each diagonal belongs to the corner it names.
 * Everything the merge did not consume is equal across the rectangle by
 * construction, so it can come from anywhere.
 */
function merged_mask(span: u32, col: u32, w: u32, h: u32): u32 {
  const near = MERGE_MASK[col];
  if (w === 1 && h === 1) return near;
  const far_x = MERGE_MASK[col + (w - 1)];
  const far_y = MERGE_MASK[col + (h - 1) * span];
  const far_xy = MERGE_MASK[col + (h - 1) * span + (w - 1)];
  const from_far = FACE_PX | CREASE_PX | RISES_PX | DIPS_PX | FACE_PY | CREASE_PY | RISES_PY | DIPS_PY |
    DIAG_PP | DIAG_PN | DIAG_NP;
  return (near & ~from_far) |
    (far_x & (FACE_PX | CREASE_PX | RISES_PX | DIPS_PX | DIAG_PN)) |
    (far_y & (FACE_PY | CREASE_PY | RISES_PY | DIPS_PY | DIAG_NP)) |
    (far_xy & DIAG_PP);
}

function runs_match(set: ColumnRunSet, a: u32, b: u32): boolean {
  return set.z0[a] === set.z0[b] && set.z1[a] === set.z1[b] &&
    set.r[a] === set.r[b] && set.g[a] === set.g[b] && set.b[a] === set.b[b] &&
    set.layer_taps[a] === set.layer_taps[b] && set.material_class[a] === set.material_class[b] &&
    set.sub_sz[a] === SUB_CELL_NONE && set.sub_sz[b] === SUB_CELL_NONE;
}

/**
 * One entry per cell: the surviving run index and its mask, or -1. At most
 * one run per cell can merge — a column with several runs keeps them all,
 * they just each stand alone, which is right: stacked runs in one column are
 * a cliff face, not flat ground.
 */
// Sized for the widest unit a scan can produce (voxel_scan.ts's
// MAX_UNIT_CHUNKS); a bake over one chunk uses the top-left corner.
const MAX_BAKE_SPAN = CHUNK_CELL_SPAN * MAX_UNIT_CHUNKS;
const MERGE_RUN = CORE_ALLOC.take(Int32Array, MAX_BAKE_SPAN * MAX_BAKE_SPAN);
const MERGE_MASK = CORE_ALLOC.take(Int32Array, MAX_BAKE_SPAN * MAX_BAKE_SPAN);

/**
 * Meshes one run set. Both LOD tiers go through here: `span` is the grid
 * width in that tier's own cells and `xy_scale` how many fine cells one
 * covers, so a coarse tier is the same algorithm on a smaller grid with
 * wider cells rather than a separate averaged-box path.
 *
 * Only X and Y scale. A run's z stays in fine cells at every tier, because
 * quantising height by the tier's own step is what makes distant terrain
 * read as flat: at lod_step 8 every rise under eight cells collapses into
 * one level. The old shrunken-box emitter hid that behind boxes of varying
 * size and the gaps between them; full-size merged boxes show it plainly.
 */
type FineBake = {
  set: ColumnRunSet;
  span: u32;
  xy_scale: u32;
  /** Where this bake's boxes start in SURVIVING. */
  base: u32;
  /** 0 mask, 1 merge, 2 emit. */
  phase: 0 | 1 | 2;
  row: u32;
  builder: MeshBuilder | null;
  emit_at: u32;
};

function begin_fine(set: ColumnRunSet, span: u32, xy_scale: u32): FineBake {
  MERGE_RUN.fill(-1);
  return { set, span, xy_scale, base: SURVIVING.len, phase: 0, row: 0, builder: null, emit_at: 0 };
}

function step_fine(b: FineBake): boolean {
  const { set, span } = b;

  if (b.phase === 0) {
    const row_end = Math.min(span, b.row + BAKE_ROWS_PER_STEP);
    for (let cy: u32 = b.row; cy < row_end; cy++) {
      for (let cx: u32 = 0; cx < span; cx++) {
        const col = cy * span + cx;
        const end = set.start[col + 1];
        let mergeable = -1;
        let mergeable_mask = 0;
        for (let i = set.start[col]; i < end; i++) {
          occlusion_stats.total_runs++;
          const mask = run_face_mask(set, span, cx, cy, i);
          occlusion_stats.faces_possible += run_include_bottom(set, i) ? FULL_FACE_CNT : OPEN_BOTTOM_FACE_CNT;
          // Concave bits shade an edge but never keep a run alive on their own.
          if ((mask & ALL_FACES) === 0) {
            occlusion_stats.culled++;
            continue;
          }
          if (set.sub_sz[i] === SUB_CELL_NONE && mergeable < 0) {
            mergeable = i;
            mergeable_mask = mask;
            continue;
          }
          // Not a merge candidate: emit it as its own single-cell box.
          push_box(cx, cy, i, mask, 1, 1);
        }
        if (mergeable >= 0) {
          MERGE_RUN[col] = mergeable;
          MERGE_MASK[col] = mergeable_mask;
        }
      }
    }
    b.row = row_end;
    if (row_end >= span) {
      b.phase = 1;
      b.row = 0;
    }
    return false;
  }

  if (b.phase === 1) {
    // Greedy rectangles over the mergeable plane: widen along X while the
    // run matches and the shared border carries no crease, then deepen along
    // Y while a whole row matches the first. Rows below the current one are
    // already fully masked, which is all a merge reads ahead of itself.
    const row_end = Math.min(span, b.row + BAKE_ROWS_PER_STEP);
    for (let cy: u32 = b.row; cy < row_end; cy++) {
      for (let cx: u32 = 0; cx < span; cx++) {
        const col = cy * span + cx;
        const run = MERGE_RUN[col];
        if (run < 0) continue;
        const mask = MERGE_MASK[col];

        let w = 1;
        while (cx + w < span) {
          // The border about to be swallowed belongs to the current far cell,
          // not to the cell this box started at. Testing `mask` instead made
          // the condition loop-invariant, so it only ever asked about the
          // first border and let the merge run through every later one.
          if ((MERGE_MASK[col + w - 1] & X_SEAM_BITS) !== 0) break;
          const next = col + w;
          const next_run = MERGE_RUN[next];
          if (next_run < 0 || !runs_match(set, run, next_run)) break;
          // Everything the merge does not consume must agree.
          if ((MERGE_MASK[next] & ~X_BORDER_BITS) !== (mask & ~X_BORDER_BITS)) break;
          w++;
        }

        let h = 1;
        outer: while (cy + h < span) {
          const row = col + h * span;
          for (let k = 0; k < w; k++) {
            if ((MERGE_MASK[col + (h - 1) * span + k] & Y_SEAM_BITS) !== 0) break outer;
            const cell = row + k;
            const cell_run = MERGE_RUN[cell];
            if (cell_run < 0 || !runs_match(set, run, cell_run)) break outer;
            if ((MERGE_MASK[cell] & ~Y_BORDER_BITS) !== (MERGE_MASK[col + k] & ~Y_BORDER_BITS)) break outer;
            // merged_mask reads the box's +Y border from column 0 of its far
            // row, and the comparison above deliberately ignores Y bits, so
            // without this the whole row has to be asked whether it agrees
            // with the column that will speak for it. It did not, and a crease
            // belonging to one column got stamped across the box's full width
            // — a line running over flat ground, starting and stopping where
            // the merge happened to.
            if ((MERGE_MASK[cell] & Y_SEAM_BITS) !== (MERGE_MASK[row] & Y_SEAM_BITS)) break outer;
          }
          h++;
        }

        for (let j = 0; j < h; j++) {
          for (let k = 0; k < w; k++) MERGE_RUN[col + j * span + k] = -1;
        }
        push_box(cx, cy, run, merged_mask(span, col, w, h), w, h);
      }
    }
    b.row = row_end;
    if (row_end < span) return false;

    // Totals are only knowable once merging is done, which is why the block
    // is sized here rather than in the pass above.
    let vertex_total: u32 = 0;
    let index_total: u32 = 0;
    for (let s_i = b.base; s_i < SURVIVING.len; s_i += 6) {
      const faces = face_cnt_of(SURVIVING.get(s_i + 3) & ALL_FACES);
      occlusion_stats.faces_kept += faces;
      vertex_total += faces * 4;
      index_total += faces * 6;
    }
    b.builder = alloc_builder(vertex_total, index_total);
    b.phase = 2;
    b.emit_at = b.base;
    return false;
  }

  const builder = b.builder!;
  const emit_end = Math.min(SURVIVING.len, b.emit_at + BAKE_BOXES_PER_STEP * 6);
  for (let s_i = b.emit_at; s_i < emit_end; s_i += 6) {
    const cx = SURVIVING.get(s_i), cy = SURVIVING.get(s_i + 1), i = SURVIVING.get(s_i + 2);
    const mask = SURVIVING.get(s_i + 3), w = SURVIVING.get(s_i + 4), h = SURVIVING.get(s_i + 5);
    const sub_sz = set.sub_sz[i];
    const unit = b.xy_scale * U_PER_CELL;
    const depth = (set.z1[i] + 1 - set.z0[i]) * U_PER_CELL;
    let min_x: f64, min_y: f64, min_z: f64, size_x: f64, size_y: f64, size_z: f64;
    if (sub_sz === SUB_CELL_NONE) {
      min_x = cx * unit;
      min_y = cy * unit;
      min_z = set.z0[i] * U_PER_CELL;
      size_x = w * unit;
      size_y = h * unit;
      size_z = depth;
    } else {
      // Inset inside its own extent, per axis, and never merged — a box that
      // does not fill its cell has no shared border to merge across, so w and
      // h are always 1 here.
      //
      // Every axis reads the same way: a fraction of the extent that axis
      // would have had. For X and Y that extent is one cell of this tier, so
      // they pick up xy_scale exactly like the full-cell branch does; for Z
      // it is the run, which is one cell for a sub-voxel object and a whole
      // tier step for a coarse bucket. No axis is privileged.
      min_x = cx * unit + unpack_sub_cell_fraction(set.sub_ox[i]) * unit;
      min_y = cy * unit + unpack_sub_cell_fraction(set.sub_oy[i]) * unit;
      min_z = set.z0[i] * U_PER_CELL + unpack_sub_cell_fraction(set.sub_oz[i]) * depth;
      size_x = unpack_sub_cell_size(set.sub_sx[i]) * unit;
      size_y = unpack_sub_cell_size(set.sub_sy[i]) * unit;
      size_z = unpack_sub_cell_size(sub_sz) * depth;
    }
    const taps = set.layer_taps[i];
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
      taps & 255,
      (taps >>> 8) & 255,
      (taps >>> 16) & 255,
      (taps >>> 24) & 255,
      mask,
    );
  }
  b.emit_at = emit_end;
  return emit_end >= SURVIVING.len;
}

function finish_fine(b: FineBake): BakedChunkMesh {
  SURVIVING.truncate(b.base);
  return finish_builder(b.builder!);
}

function abort_fine(b: FineBake): void {
  if (b.builder) CORE_POOL.release(b.builder.block);
  SURVIVING.truncate(b.base);
}

/** Records one box to emit: cell origin, its run, its mask, and how many
 * cells it spans. Stride 6 in the same scratch the pre-merge pass used. */
function push_box(cx: u32, cy: u32, run: u32, mask: u32, w: u32, h: u32): void {
  SURVIVING.push(cx);
  SURVIVING.push(cy);
  SURVIVING.push(run);
  SURVIVING.push(mask);
  SURVIVING.push(w);
  SURVIVING.push(h);
}

/**
 * One coarse cell's contents at one `lod_step`-cubed lattice cell.
 *
 * `bz` names the cell; its extent is `lod_step` fine cells on every axis,
 * and `coverage` counts the fine voxels inside it. Occupancy is that count
 * over `lod_step^3` — one number, no axis singled out — and it shrinks the
 * emitted box isotropically about the cell's own centre.
 *
 * The colour is not volumetric: `sr/sg/sb`
 * sum only the *topmost* run of each sub-column, because that is the voxel
 * you can see. Averaging by volume instead makes a cell of grass over twenty
 * of rock come out almost entirely rock, and distant terrain converge to a
 * muddy average of everything buried under it.
 *
 * `tag`/`tr`/`tg`/`tb` are how that stays O(1) per run: runs arrive
 * ascending within a sub-column, so a later one supersedes the one before
 * it, and the sum is corrected by subtracting what that column last
 * contributed rather than by rescanning.
 */
type ZBucketAccum = {
  bz: i32;
  coverage: f64;
  sr: f64;
  sg: f64;
  sb: f64;
  surfaces: u32;
  tag: u32;
  tr: f64;
  tg: f64;
  tb: f64;
  taps: u32;
  tap_cnt: u32;
};

/**
 * Every coarse cell's Z buckets, bump-allocated and rewound per chunk.
 * `bake_chunk_mesh_coarse` fills one cell completely before starting the
 * next, so a cell's buckets are contiguous here and CELL_RANGE below only
 * has to remember (start, count) — no per-cell container at all, where the
 * previous shape allocated `res^2` arrays plus an object per bucket on every
 * single rebuild.
 */
const BUCKETS = new Arena<ZBucketAccum>(
  () => ({
    bz: 0,
    coverage: 0,
    sr: 0,
    sg: 0,
    sb: 0,
    surfaces: 0,
    tag: 0,
    tr: 0,
    tg: 0,
    tb: 0,
    taps: 0,
    tap_cnt: 0,
  }),
  { init_cap: 1 << 12 },
);
const CELL_RANGE = new Vector(Uint32Array, { init_cap: 2 * (1 << 10), alloc: CORE_ALLOC });

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
  acc.sr = 0;
  acc.sg = 0;
  acc.sb = 0;
  acc.surfaces = 0;
  acc.tag = 0;
  acc.coverage = 0;
  acc.taps = 0;
  acc.tap_cnt = 0;
  return BUCKETS.len - 1;
}

/** Folds one sub-column's contribution to this bucket's visible surface,
 * superseding whatever that same column contributed lower down. */
function collect_surface(acc: ZBucketAccum, tag: u32, r: f64, g: f64, b: f64): void {
  if (acc.tag === tag) {
    acc.sr += r - acc.tr;
    acc.sg += g - acc.tg;
    acc.sb += b - acc.tb;
  } else {
    acc.sr += r;
    acc.sg += g;
    acc.sb += b;
    acc.surfaces++;
    acc.tag = tag;
  }
  acc.tr = r;
  acc.tg = g;
  acc.tb = b;
}

/**
 * Up to four distinct texture layers touching this bucket, packed one per
 * byte in first-encountered order. The shader averages the four at 25% each,
 * so a coarse cell spanning grass, road and roof blends them rather than
 * showing whichever it met first. Beyond four, the first four win.
 *
 * Small-N linear scan against the bytes already packed: a bucket sees a
 * handful of materials, nowhere near enough to justify anything else.
 */
function collect_layer_tap(acc: ZBucketAccum, layer_idx: u32): void {
  if (layer_idx === NO_LAYER || acc.tap_cnt >= 4) return;
  for (let i: u32 = 0; i < acc.tap_cnt; i++) {
    if (((acc.taps >>> (i * 8)) & 255) === layer_idx) return;
  }
  acc.taps |= layer_idx << (acc.tap_cnt * 8);
  acc.tap_cnt++;
}

/** The packed vec4 of taps, cycling the real ones into every slot. Padding
 * unused slots with NO_LAYER would dilute toward flat colour whenever fewer
 * than four materials touched a bucket, which is the common case. */
function bucket_taps(acc: ZBucketAccum): u32 {
  if (acc.tap_cnt === 0) return NO_LAYER * 0x01010101;
  let packed = 0;
  for (let i: u32 = 0; i < 4; i++) {
    packed |= ((acc.taps >>> ((i % acc.tap_cnt) * 8)) & 255) << (i * 8);
  }
  return packed >>> 0;
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
function accumulate_run(start: u32, cnt: u32, lod_step: LodStep, set: ColumnRunSet, i: u32, tag: u32): u32 {
  const run_z0 = set.z0[i], run_z1 = set.z1[i];
  // A fine run packs one layer into all four taps, so any byte is it.
  const run_r = set.r[i], run_g = set.g[i], run_b = set.b[i], run_layer = set.layer_taps[i] & 255;
  if (set.sub_sz[i] !== SUB_CELL_NONE) {
    const frac = unpack_sub_cell_size(set.sub_sz[i]);
    const w = frac * frac * frac;
    if (w <= 0) return cnt;
    const idx = find_or_create_bucket(start, cnt, Math.floor(run_z0 / lod_step));
    const acc = BUCKETS.get(idx);
    acc.coverage += w;
    collect_surface(acc, tag, run_r, run_g, run_b);
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
    acc.coverage += overlap;
    collect_surface(acc, tag, run_r, run_g, run_b);
    collect_layer_tap(acc, run_layer);
  }
  return cnt;
}

/** One coarse run, before it is packed into an exactly-sized set. Same
 * scratch-then-copy shape voxel_scan.ts uses, for the same reason: the run
 * count is only known once the walk is done. */
const CO_Z0 = new Vector(Int32Array, { init_cap: 1 << 11, alloc: CORE_ALLOC });
const CO_Z1 = new Vector(Int32Array, { init_cap: 1 << 11, alloc: CORE_ALLOC });
const CO_RGB = new Vector(Uint32Array, { init_cap: 1 << 11, alloc: CORE_ALLOC });
const CO_LAYER = new Vector(Uint32Array, { init_cap: 1 << 11, alloc: CORE_ALLOC });
/** The run's isotropic inset, packed `offset | scale<<8`, or 0 for a box
 * that fills its cell — which is SUB_CELL_NONE in both bytes, so zero is
 * unambiguous. */
const CO_SUB = new Vector(Uint32Array, { init_cap: 1 << 11, alloc: CORE_ALLOC });
const CO_START = CORE_ALLOC.take(Uint32Array, MAX_BAKE_SPAN * MAX_BAKE_SPAN + 1);

function to_u8(v: f64): u32 {
  return Math.max(0, Math.min(255, Math.round(v)));
}

/**
 * Appends one coarse cell's Z buckets to the scratch above, ascending.
 *
 * Buckets are created in the order runs are encountered across lod_step^2
 * fine columns, so they are not sorted by height, and a run set has to be:
 * chunk_mesher's own neighbour scans stop on the first run starting above
 * the range they care about. Hence the selection walk rather than a sort —
 * a cell holds a handful of buckets, bounded by how many bands actually
 * stack under it.
 *
 * Vertically adjacent buckets of the same quantised colour *and the same XY
 * box* become one run, which is what lets a coarse cliff be one tall box
 * rather than a stack of one-deep ones. The box has to match too now that it
 * varies per bucket: stacking two different footprints into one run would
 * draw whichever one the lower bucket happened to have.
 */
function pack_coarse_cell(start: u32, cnt: u32, lod_step: LodStep): void {
  let open = -1;
  let last_top = 0;
  let previous_bz = -0x7fffffff;

  for (let seen = 0; seen < cnt; seen++) {
    let pick = -1;
    let pick_bz = 0;
    for (let i = start; i < start + cnt; i++) {
      const bz = BUCKETS.get(i).bz;
      if (bz <= previous_bz) continue;
      if (pick < 0 || bz < pick_bz) {
        pick = i;
        pick_bz = bz;
      }
    }
    if (pick < 0) break;
    previous_bz = pick_bz;

    const acc = BUCKETS.get(pick);
    const bucket_lo = pick_bz * lod_step;
    const bucket_hi = bucket_lo + lod_step - 1;
    const fill: f64 = Math.min(1, acc.coverage / (lod_step * lod_step * lod_step));
    coarse_instance_stats.total++;
    if (fill < LOW_FILL_THRESHOLD) coarse_instance_stats.low_fill++;

    // Cube root, so the box's volume is the fill fraction of the cell's.
    const linear = Math.cbrt(fill);
    const sub = linear >= 1 ? 0 : pack_inset(linear);
    const n = Math.max(1, acc.surfaces);
    const rgb = (to_u8(acc.sr / n) << 16) | (to_u8(acc.sg / n) << 8) | to_u8(acc.sb / n);
    const taps = bucket_taps(acc);

    // Only full cells stack. Two shrunken boxes are two boxes with air
    // between them, which is the whole point of shrinking them.
    if (
      sub === 0 && open >= 0 && bucket_lo === last_top + 1 &&
      CO_RGB.get(open) === rgb && CO_LAYER.get(open) === taps && CO_SUB.get(open) === 0
    ) {
      CO_Z1.set(open, bucket_hi);
      last_top = bucket_hi;
      continue;
    }
    open = CO_Z0.push(bucket_lo);
    CO_Z1.push(bucket_hi);
    CO_RGB.push(rgb);
    CO_LAYER.push(taps);
    CO_SUB.push(sub);
    last_top = bucket_hi;
  }
}

/**
 * One isotropic scale and the centring offset that goes with it, packed
 * `o | s<<8` and used for all three axes.
 *
 * Both are fractions of the extent the box would otherwise have had, which
 * is what makes one packed value serve X, Y and Z: the cell is a cube in
 * cells, so the same pair centres the box on every axis.
 */
function pack_inset(linear: f64): u32 {
  return (pack_sub_cell_fraction((1 - linear) * 0.5) | (pack_sub_cell_fraction(linear) << 8)) >>> 0;
}

/**
 * Coarse LOD, and the whole of what a merged cell is allowed to say.
 *
 * A coarse cell is `lod_step^3` fine voxels collapsed into one box, which
 * throws away everything except three numbers. Those three are the model,
 * and every question about this tier resolves to which one is being asked:
 *
 *   **How much is there → the box's volume.** Occupancy is `coverage /
 *   lod_step^3`, one scalar, and the box's side is its *cube root*, so the
 *   box's volume is that fraction of the cell's. Size rather than alpha
 *   because alpha in a voxel renderer buys a sorting problem to express
 *   something that is not actually translucent — a quarter-full cell is a
 *   small solid thing, not a see-through big one.
 *
 *   **What it looks like → the average of the surfaces you can see.** Only
 *   the topmost run of each sub-column contributes (see ZBucketAccum): a
 *   cell of grass over twenty of rock is grass, because grass is all that
 *   is visible from above. Worth knowing the limit — "visible" here means
 *   visible *downward*, so a cliff's side voxels never colour anything.
 *
 *   **What it is made of → up to four texture layers, blended.** A separate
 *   channel from the average colour, and not redundant with it: without it
 *   a cell spanning grass, road and roof shows whichever it met first.
 *
 * Then: **the box is centred in its cell on all three axes.** The lattice
 * is a cube and the filter is isotropic — X, Y and Z all decimate by
 * `lod_step` — so the operator commutes with a rotation that swaps two
 * axes. Mesh a chunk on its side and you get the same chunk on its side.
 *
 * Nothing else is expressed and nothing is discarded. There is no occupancy
 * threshold: a cell keeps shrinking however sparse it gets, so seven VOID
 * and one red block is one quarter-size red cube, with no alpha, no cull
 * and no dilation. `LOW_FILL_THRESHOLD` only counts.
 *
 * The isotropy costs something real and it is worth knowing what. A surface
 * has low volumetric fill in every cell it crosses, so flat ground shrinks
 * too and distant terrain reads as separated cubes rather than a continuous
 * sheet. That look is the point: a rule that stopped applying to the axis
 * the data happens to vary least in would not be carrying density any more,
 * just height — and which axis that is, is a fact about today's camera, not
 * about voxels.
 *
 * Mechanically this is the mipmap/voxel-cone-tracing screen-footprint
 * filter, not nearest-neighbour: colour mixing across material boundaries
 * is intentional. An object straddling a fixed cell boundary still splits
 * across two smaller boxes rather than one full one — an accepted side
 * effect of a fixed grid, not the old hard-threshold disappearing bug.
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
  return bake_chunk_mesh(set, lod_step);
}

type CoarseBake = {
  set: ColumnRunSet;
  lod_step: LodStep;
  /** Coarse cells per axis. */
  res: u32;
  bucket_base: u32;
  range_base: u32;
  /** 0 accumulate, 1 pack, 2 the fine bake over the packed result. */
  phase: 0 | 1 | 2;
  row: u32;
  coarse: ColumnRunSet | null;
  fine: FineBake | null;
};

function begin_coarse(set: ColumnRunSet, lod_step: LodStep): CoarseBake {
  if (lod_step === 1) throw new Error("render: finest LOD requires per-run meshing");
  return {
    set,
    lod_step,
    res: set.span / lod_step,
    bucket_base: BUCKETS.len,
    range_base: CELL_RANGE.len,
    phase: 0,
    row: 0,
    coarse: null,
    fine: null,
  };
}

/** Coarse rows a step covers. Each spans lod_step fine rows, so this keeps a
 * step to the same amount of fine-column work as a fine bake's step. */
function coarse_rows_per_step(lod_step: LodStep): u32 {
  return Math.max(1, Math.floor(BAKE_ROWS_PER_STEP / lod_step));
}

function step_coarse(b: CoarseBake): boolean {
  const { set, lod_step, res } = b;

  if (b.phase === 0) {
    const row_end = Math.min(res, b.row + coarse_rows_per_step(lod_step));
    for (let by: u32 = b.row; by < row_end; by++) {
      for (let bx: u32 = 0; bx < res; bx++) {
        const start = BUCKETS.len;
        let cnt: u32 = 0;
        for (let j: u32 = 0; j < lod_step; j++) {
          const cy = by * lod_step + j;
          for (let k: u32 = 0; k < lod_step; k++) {
            const cx = bx * lod_step + k;
            const col = cy * set.span + cx;
            const end = set.start[col + 1];
            // col + 1 so no sub-column ever carries tag 0, which is what a
            // freshly allocated bucket holds.
            for (let i = set.start[col]; i < end; i++) cnt = accumulate_run(start, cnt, lod_step, set, i, col + 1);
          }
        }
        CELL_RANGE.push(start);
        CELL_RANGE.push(cnt);
      }
    }
    b.row = row_end;
    if (row_end >= res) {
      b.phase = 1;
      b.row = 0;
      CO_Z0.truncate(0);
      CO_Z1.truncate(0);
      CO_RGB.truncate(0);
      CO_LAYER.truncate(0);
      CO_SUB.truncate(0);
    }
    return false;
  }

  if (b.phase === 1) {
    // The averaged buckets become a run set of their own, on a res x res grid
    // whose cells are lod_step fine cells wide, and then go through the same
    // mesher the finest tier uses. Coarse geometry gets neighbour face
    // culling and greedy merging for free that way, and distant terrain ends
    // up speaking the same visual language as near terrain — merged flat
    // surfaces with silhouette shading — rather than a field of separate
    // boxes.
    const row_end = Math.min(res, b.row + coarse_rows_per_step(lod_step));
    for (let c: u32 = b.row * res; c < row_end * res; c++) {
      CO_START[c] = CO_Z0.len;
      const ri = b.range_base + c * 2;
      pack_coarse_cell(CELL_RANGE.get(ri), CELL_RANGE.get(ri + 1), lod_step);
    }
    b.row = row_end;
    if (row_end < res) return false;

    CO_START[res * res] = CO_Z0.len;
    const coarse = alloc_column_run_set(CO_Z0.len);
    coarse.start.set(CO_START.subarray(0, res * res + 1));
    for (let i: u32 = 0; i < CO_Z0.len; i++) {
      const rgb = CO_RGB.get(i);
      coarse.z0[i] = CO_Z0.get(i);
      coarse.z1[i] = CO_Z1.get(i);
      coarse.r[i] = (rgb >>> 16) & 255;
      coarse.g[i] = (rgb >>> 8) & 255;
      coarse.b[i] = rgb & 255;
      coarse.layer_taps[i] = CO_LAYER.get(i);
      coarse.material_class[i] = MaterialClass.LAND;
      const sub = CO_SUB.get(i);
      const inset = sub & 255, scale = (sub >>> 8) & 255;
      coarse.sub_sz[i] = sub === 0 ? SUB_CELL_NONE : scale;
      coarse.sub_sx[i] = scale;
      coarse.sub_sy[i] = scale;
      coarse.sub_ox[i] = inset;
      coarse.sub_oy[i] = inset;
      coarse.sub_oz[i] = inset;
    }
    CELL_RANGE.truncate(b.range_base);
    BUCKETS.truncate(b.bucket_base);
    b.coarse = coarse;
    b.fine = begin_fine(coarse, res, lod_step);
    b.phase = 2;
    return false;
  }

  return step_fine(b.fine!);
}

function finish_coarse(b: CoarseBake): BakedChunkMesh {
  const baked = finish_fine(b.fine!);
  release_column_runs(b.coarse!);
  return baked;
}

function abort_coarse(b: CoarseBake): void {
  if (b.fine) abort_fine(b.fine);
  if (b.coarse) release_column_runs(b.coarse);
  CELL_RANGE.truncate(b.range_base);
  BUCKETS.truncate(b.bucket_base);
}
