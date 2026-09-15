// @pxl/web/reference/mesh/mesh_block.ts

/**
 * A chunk mesh's block as the mesher lays it out
 * (crates/pxl/core/src/bake/mesher.rs, and reference/chunk_mesher.ts held
 * to it): every lane of a baked mesh in one block, in this fixed order,
 * then the visibility lane, then the index lane. They share a lifetime
 * exactly (the whole mesh dies at once, when its owner does), so one
 * allocation rather than eight — measured: a zoom-out that rebuilds ~2000
 * chunks moved +344MB in a single frame, 8.4s of it off-thread, when each
 * lane was its own. The renderer reads the same layout from the pipeline's
 * record (crates/pxl/core/src/render/shader.rs); this is the glue's view of it.
 */

import type { u32 } from "@xpute/core/kit/word.ts";
import type { Block } from "@xpute/core/ds/buddy.ts";
import { carver } from "@xpute/core/ds/typed_alloc.ts";

/** A tier as the module hands it over and the glue holds it: the block in
 * the mesh section and the counts its lanes are read by. The renderer
 * uploads the block whole by its address and reads no lane; the lanes are
 * carved (below) only where something reads them — the reference tests. */
export type MeshBlock = {
  block: Block;
  vertex_cnt: u32;
  index_cnt: u32;
};

/**
 * One baked chunk mesh's lanes — position/normal are the real chunk-local
 * vertices every box's own faces actually sit at; the rest are per-vertex
 * material/shading inputs the chunk shader reads directly
 * (crates/pxl/core/src/render/shader/chunk.wgsl; `local_pos`/`box_scale`
 * replace what an instanced approach would reconstruct from an instance
 * matrix). `visibility` is deliberately NOT included here: the mesher only
 * ever bakes real geometry, never a per-role default (chunk_bucket.ts's
 * with_visibility owns that, sized by vertex_cnt).
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

const F32_PER_VERTEX: u32 = 3 /* position */ + 3 /* normal */ + 3 /* local_pos */ + 3 /* box_scale */ + 3 /* rgb */ + 4 /* layer_taps */ + 1 /* faces */;
const VIS_F32_PER_VERTEX: u32 = 1;

function block_bytes(vertex_total: u32, index_total: u32): u32 {
  return (vertex_total * (F32_PER_VERTEX + VIS_F32_PER_VERTEX) + index_total) * 4;
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

/** The bytes one mesh's block holds, visibility's span included. What a
 * mesh copied across a thread boundary has to carry. */
export function baked_mesh_bytes(vertex_cnt: u32, index_cnt: u32): u32 {
  return block_bytes(vertex_cnt, index_cnt);
}

/**
 * A mesh's views over a block laid out by this module — the one it was
 * baked into, or a copy of those bytes somewhere else. Every span is
 * measured from the block's own start, never the buffer's: a region is
 * shared with other live chunks' blocks.
 */
export function carve_baked_mesh(block: Block, vertex_cnt: u32, index_cnt: u32): BakedChunkMesh {
  const cut = carver(block.buf, block.byte_offset);
  return {
    block,
    vertex_cnt,
    position: cut(Float32Array, vertex_cnt * 3),
    normal: cut(Float32Array, vertex_cnt * 3),
    local_pos: cut(Float32Array, vertex_cnt * 3),
    box_scale: cut(Float32Array, vertex_cnt * 3),
    rgb: cut(Float32Array, vertex_cnt * 3),
    layer_taps: cut(Float32Array, vertex_cnt * 4),
    faces: cut(Float32Array, vertex_cnt),
    // visibility's own span sits here, carved on demand by carve_visibility.
    index: new Uint32Array(block.buf, block.byte_offset + vertex_cnt * (F32_PER_VERTEX + VIS_F32_PER_VERTEX) * 4, index_cnt),
  };
}
