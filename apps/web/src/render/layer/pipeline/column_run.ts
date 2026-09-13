// @pxl/web/render/layer/pipeline/column_run.ts

/**
 * The run set's layout: which spans a block holds, in what order, and how
 * many bytes that takes. No allocation of its own — a block is handed in —
 * so the main thread carves one from the core pool and a worker carves one
 * from its own memory or from a slab it was sent, with the same layout on
 * both sides. A buddy block starts on an 8-aligned offset, and so does a
 * slab, so a set copied between them re-carves to the same views.
 */

import type { u32 } from "@xpute/core/kit/word.ts";
import type { Block } from "@xpute/core/ds/buddy.ts";
import { carver } from "@xpute/core/ds/typed_alloc.ts";

/**
 * Every column's runs for one chunk, laid out as parallel arrays over a
 * single pooled block.
 *
 * This was `MeshRun[][]`: 4096 arrays plus an object per run, ~10k objects
 * and about a megabyte for a dense chunk, *retained* for as long as the
 * chunk keeps its scan. Measured, it was the largest remaining allocator
 * once the vertex blocks were pooled — a committing frame moved +1.43MB
 * with nothing else left to account for it.
 *
 * A run is 17 bytes here against roughly a hundred as an object, and one
 * allocation replaces ten thousand. The fields are split by width rather
 * than interleaved because that is what makes them typed-array spans at all;
 * nothing reads a whole run at once anyway.
 *
 * `start` is a prefix index: column c's runs are `[start[c], start[c + 1])`,
 * which is what lets the whole thing be flat — a column's runs are
 * contiguous because merge_column_runs finishes one column before the next.
 */
export type ColumnRunSet = {
  block: Block;
  /** Total runs across every column. */
  count: u32;
  /** Columns on a side: 64 for a chunk, fewer for a coarse tier's own grid.
   * Column `(cx, cy)` is `cy * span + cx`, which is how the mesher reads it. */
  span: u32;
  /** span² + 1 offsets into the per-run spans below. */
  start: Uint32Array;
  z0: Int32Array;
  z1: Int32Array;
  r: Uint8Array;
  g: Uint8Array;
  b: Uint8Array;
  /**
   * Four texture-array layers packed one per byte, which the shader averages
   * at 25% each.
   *
   * Four rather than one because a coarse tier's cell can span several
   * materials and showing only the first reads as blotchy: the taps are how
   * a merged cell blends grass into road into roof. A fine run has one
   * material, so it writes the same layer into all four and the average is
   * that layer — which is what the finest tier has always done.
   */
  layer_taps: Uint32Array;
  material_class: Uint8Array;
  /**
   * SUB_CELL_NONE (0) for a full-cell run; otherwise the run's box is inset
   * inside its cell and `sub_sx`/`sub_sy`/`sub_sz` give its extent per axis.
   *
   * `sub_sz` is the discriminator for all three, and it scales the run's
   * whole z extent rather than one cell — for a sub-voxel object the run is
   * one cell tall so the two agree, and for a coarse LOD cell it is what
   * lets the box keep an exact multi-cell height while shrinking in XY.
   *
   * Per-axis rather than one isotropic size because sparsity has a
   * direction: a lone pillar under a coarse cell is narrow in XY and full
   * height, and a single scale factor can only say one of those two things.
   */
  sub_sz: Uint8Array;
  sub_sx: Uint8Array;
  sub_sy: Uint8Array;
  sub_ox: Uint8Array;
  sub_oy: Uint8Array;
  sub_oz: Uint8Array;
};

const RUN_I32_FIELDS: u32 = 3; // z0, z1, layer_taps
const RUN_U8_FIELDS: u32 = 10; // r g b material_class sub_sz sub_sx sub_sy sub_ox sub_oy sub_oz

export function column_run_set_bytes(count: u32, span: u32): u32 {
  return (span * span + 1) * 4 + count * (RUN_I32_FIELDS * 4 + RUN_U8_FIELDS);
}

/** Views over `block` for a set of `count` runs on a `span`² grid. The block
 * must hold at least column_run_set_bytes(count, span). */
export function carve_column_run_set(block: Block, count: u32, span: u32): ColumnRunSet {
  const cut = carver(block.buf, block.byte_offset);
  return {
    block,
    count,
    span,
    start: cut(Uint32Array, span * span + 1),
    z0: cut(Int32Array, count),
    z1: cut(Int32Array, count),
    layer_taps: cut(Uint32Array, count),
    r: cut(Uint8Array, count),
    g: cut(Uint8Array, count),
    b: cut(Uint8Array, count),
    material_class: cut(Uint8Array, count),
    sub_sz: cut(Uint8Array, count),
    sub_sx: cut(Uint8Array, count),
    sub_sy: cut(Uint8Array, count),
    sub_ox: cut(Uint8Array, count),
    sub_oy: cut(Uint8Array, count),
    sub_oz: cut(Uint8Array, count),
  };
}
