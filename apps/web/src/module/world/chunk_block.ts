// @pxl/web/module/world/chunk_block.ts

/**
 * A block: the unit of cube work at a coarse tier.
 *
 * At step 8 and 16 a chunk no longer scans and bakes its own cubes. It is a
 * member of a block covering `unit × unit` chunks that does so once for all
 * of them, so the cost per unit of work stays flat across tiers instead of
 * growing sixteenfold at the tier that draws least
 * (issue/20260912-coarse-tiers-need-a-larger-unit.md).
 *
 * A block has no scene presence and no scheduling entry of its own. Its
 * member chunks drive it: whichever member commits first advances the scan
 * and the bake, using the same two global slots and the same forward-progress
 * rule a chunk's own build uses, and every member reads the result. The mesh
 * is adopted chunk by chunk — each member selects its own 64×64 region of it
 * through the per-vertex visibility the dither blend already uses — so the
 * handoff rule "keep drawing what you have until the replacement is ready"
 * holds per chunk and no four-way swap exists.
 *
 * Blocks live in one registry keyed by unit origin; a member links on its
 * first coarse commit and unlinks when it refines or disposes, and a block
 * with no members gives its memory back.
 */

import type { f64, u32 } from "@xpute/core/kit/word.ts";
import { CHUNK_CELL_SPAN } from "@pxl/shared/bake/constant.ts";
import { ivec } from "@pxl/shared/spec/space/pvf3/mod.ts";

import { composite_chunk_layers } from "@/render/layer/pipeline/compositor.ts";
import type { ChunkKey, WorldLayerStack } from "@/render/layer/pipeline/contract.ts";
import { abort_bake, bake_open, type BakeCursor, begin_bake, finish_bake, step_bake } from "@/render/layer/pipeline/chunk_mesher.ts";
import { bucket_from_baked, type BuiltChunkBucket, release_built_mesh, retain_built_mesh } from "@/render/layer/pipeline/chunk_bucket.ts";
import {
  abort_column_scan,
  begin_column_scan,
  column_scan_open,
  type ColumnRunSet,
  type ColumnScan,
  finish_column_scan,
  MAX_UNIT_CHUNKS,
  release_column_runs,
  scan_column_rows,
  scan_next_row,
  scan_wants_row,
} from "@/render/layer/pipeline/voxel_scan.ts";
import { type ChunkVolume, release_chunk_volume } from "@/render/layer/pipeline/volume.ts";
import type { FrameBudget } from "@/module/world/sched/tick.ts";
import { lod_build_stats } from "@/module/world/lod_build_stat.ts";
import type { LodStep } from "@/render/lod.ts";

/** Chunks on a side of the unit a coarse step works on: the step's own
 * stride, so the unit's cell count stays constant across tiers, capped at
 * the widest unit the scan can hold (MAX_UNIT_CHUNKS). Steps below
 * BLOCK_MIN_STEP keep the chunk as their unit. The cap matters for steps 32
 * and 64, which the range never shows but which the ladder can still name
 * — a chunk's first seed before it has bounds, say. */
export const BLOCK_MIN_STEP: LodStep = 8;

export function block_unit_of(step: LodStep): u32 {
  return step >= BLOCK_MIN_STEP ? Math.min(step / 4, MAX_UNIT_CHUNKS) : 1;
}

/** Whether cubes at this step are baked by a block rather than the chunk. */
export function step_uses_block(step: LodStep): boolean {
  return step >= BLOCK_MIN_STEP;
}

const CHUNK_MASK = ~(BigInt(CHUNK_CELL_SPAN) - 1n);

/**
 * The unit a chunk belongs to at a step: the chunk's page and the page
 * offset of the unit's first chunk. A unit never straddles a page — sixteen
 * chunks on a side, and a unit is at most four — so the page is the chunk's.
 */
export function block_key_of(key: ChunkKey, step: LodStep): ChunkKey {
  const unit = block_unit_of(step);
  const align = ~(BigInt(unit * CHUNK_CELL_SPAN) - 1n);
  return { page_idx: key.page_idx, page_off: ivec.gather(ivec.X(key.page_off) & align, ivec.Y(key.page_off) & align, 0n) };
}

/** A chunk's position inside its unit, in chunks. */
export function block_member_index(key: ChunkKey, step: LodStep): { ux: u32; uy: u32 } {
  const unit = block_unit_of(step);
  const origin = block_key_of(key, step).page_off;
  return {
    ux: Number((ivec.X(key.page_off) - ivec.X(origin)) / BigInt(CHUNK_CELL_SPAN)) % unit,
    uy: Number((ivec.Y(key.page_off) - ivec.Y(origin)) / BigInt(CHUNK_CELL_SPAN)) % unit,
  };
}

function registry_key(key: ChunkKey, unit: u32): string {
  return `${key.page_idx}:${ivec.X(key.page_off) & CHUNK_MASK},${ivec.Y(key.page_off) & CHUNK_MASK}:${unit}`;
}

type ScanInFlight = {
  cursor: ColumnScan;
  /** The chunk-row of composites the scan is walking now. */
  row: (ChunkVolume | null)[];
  next_chunk_row: u32;
  spent_ms: f64;
};

type BakeInFlight = { step: LodStep; cursor: BakeCursor; spent_ms: f64 };

export class ChunkBlock {
  readonly key: ChunkKey;
  readonly unit: u32;
  readonly span: u32;
  /**
   * Scene presence is the members' business, not the block's: this module
   * knows nothing of three.js so the bake can be tested without a renderer.
   * The first member to see a step baked mounts its mesh (ChunkNode), and
   * registers here so a reveal can flag the attribute; the mount is dropped
   * when the tier is cleared. `on_cleared` lets the mounter take it down.
   */
  private readonly mounts = new Map<LodStep, { flag: () => void; unmount: () => void }>();

  private column_runs: ColumnRunSet | null = null;
  private scan: ScanInFlight | null = null;
  private bake: BakeInFlight | null = null;
  private readonly baked = new Map<LodStep, BuiltChunkBucket>();
  private members: u32 = 0;
  /** Members to tell when the bake is cleared, so each can mark its layout
   * not ready and rebuild through the block on its next commit. */
  private readonly listeners = new Set<() => void>();

  constructor(key: ChunkKey, step: LodStep, private readonly stack: WorldLayerStack) {
    this.key = key;
    this.unit = block_unit_of(step);
    this.span = this.unit * CHUNK_CELL_SPAN;
  }

  get holds_slot(): boolean {
    return this.scan !== null || this.bake !== null;
  }

  /** Bytes of built work this block keeps: its run set and its baked tiers. */
  get retained(): u32 {
    let bytes = this.column_runs?.block.bytes ?? 0;
    for (const bucket of this.baked.values()) bytes += bucket.cubes.block.bytes;
    return bytes;
  }

  /** A member's source changed: the run set is out of date. The current
   * scan, if any, is dropped rather than finished on stale input, and every
   * member is told so its layout goes back through the block. */
  invalidate(): void {
    this.abort_work();
    this.clear_column_runs();
    this.clear_baked();
    for (const fn of this.listeners) fn();
  }

  /** Registers a member's reaction to `invalidate`; returns the unsubscribe. */
  on_invalidate(fn: () => void): () => void {
    this.listeners.add(fn);
    return () => this.listeners.delete(fn);
  }

  private clear_baked(): void {
    for (const [step, bucket] of this.baked) {
      this.unmount(step);
      this.baked.delete(step);
      release_built_mesh(bucket.cubes);
    }
  }

  /** Whether a step's mesh has been mounted by some member. */
  mounted(step: LodStep): boolean {
    return this.mounts.has(step);
  }

  /** Registers the member that mounted a step's mesh: how to flag its
   * visibility attribute after a reveal, and how to take it down. */
  register_mount(step: LodStep, flag: () => void, unmount: () => void): void {
    this.mounts.set(step, { flag, unmount });
  }

  private unmount(step: LodStep): void {
    const m = this.mounts.get(step);
    if (!m) return;
    m.unmount();
    this.mounts.delete(step);
  }

  /**
   * A member's cube contribution at a coarse step: its region of the block
   * mesh, shown at `value` (the same encoding the dither blend uses, 1 for
   * everything, 0 for nothing). The mesh must be baked at `step`.
   */
  reveal(step: LodStep, ux: u32, uy: u32, value: f64): boolean {
    const bucket = this.baked.get(step);
    // Nothing to reveal is not an error here: a member's per-frame decision
    // can run between an invalidate and the member's next commit, and its
    // region simply no longer exists until the block bakes again.
    if (!bucket) return false;
    block_reveal_region(bucket, ux, uy, value);
    this.mounts.get(step)?.flag();
    return true;
  }

  /**
   * Advances toward a baked mesh at `step`, under `budget`, and says where
   * it got to — the same three answers a chunk's own build gives. "done"
   * means `bucket_at(step)` is ready.
   */
  advance(step: LodStep, budget: FrameBudget | null): "done" | "busy" | "blocked" {
    if (this.baked.has(step)) return "done";
    if (!this.column_runs) {
      const state = this.advance_scan(budget);
      if (state !== "done") return state;
    }
    return this.advance_bake(step, budget);
  }

  /** The baked mesh at `step`, once `advance` said "done". */
  bucket_at(step: LodStep): BuiltChunkBucket | null {
    return this.baked.get(step) ?? null;
  }

  /**
   * Takes a reference to the mesh at `step` for a member that adopts it. The
   * member owns one reference per adoption and releases it with `disown`.
   */
  adopt(step: LodStep): BuiltChunkBucket {
    const bucket = this.baked.get(step);
    if (!bucket) throw new Error("render: adopting a block tier that is not baked");
    retain_built_mesh(bucket.cubes);
    return bucket;
  }

  disown(bucket: BuiltChunkBucket): void {
    release_built_mesh(bucket.cubes);
  }

  /** @internal block_for links; members never call this directly. */
  link(): void {
    this.members++;
  }

  /**
   * Returns true when this was the last member and the block is empty.
   *
   * Any member leaving yields the block's in-flight work, not only the
   * last. A block is driven by whichever member commits, so the member that
   * just left may have been the only one driving it; a scan left open with
   * no driver holds the one scan slot for the whole world until something
   * happens to advance it, which is nothing. Three of those were found in
   * one afternoon (2026-09-12) on the refine path, the release path and by
   * the frustum; one rule here covers every path. The baked mesh holds no
   * slot and stays for the members that remain.
   */
  unlink(): boolean {
    this.members--;
    this.abort_work();
    if (this.members > 0) return false;
    this.clear_column_runs();
    this.clear_baked();
    return true;
  }

  private member_key(ux: u32, uy: u32): ChunkKey {
    return {
      page_idx: this.key.page_idx,
      page_off: ivec.add(this.key.page_off, ivec.gather(BigInt(ux * CHUNK_CELL_SPAN), BigInt(uy * CHUNK_CELL_SPAN), 0n)),
    };
  }

  /** Composites one chunk-row of the unit, null for a chunk with nothing. */
  private composite_row(uy: u32): (ChunkVolume | null)[] {
    const row: (ChunkVolume | null)[] = [];
    for (let ux: u32 = 0; ux < this.unit; ux++) row.push(composite_chunk_layers(this.stack, this.member_key(ux, uy)));
    return row;
  }

  private release_row(row: (ChunkVolume | null)[]): void {
    for (const v of row) if (v) release_chunk_volume(v);
  }

  private advance_scan(budget: FrameBudget | null): "done" | "busy" | "blocked" {
    const t0 = performance.now();
    if (!this.scan) {
      if (column_scan_open()) return "blocked";
      const row = this.composite_row(0);
      const cursor = begin_column_scan(row, this.span);
      if (cursor === null) {
        this.release_row(row);
        return "blocked";
      }
      this.scan = { cursor, row, next_chunk_row: 1, spent_ms: 0 };
    }

    // At least one step, then as many as the budget allows; between
    // chunk-rows the next row's composites are made on the spot. A unit with
    // nothing in a whole chunk-row still has to walk it, for the prefix.
    let done: boolean;
    do {
      done = scan_column_rows(this.scan.cursor);
      if (!done && scan_wants_row(this.scan.cursor)) {
        this.release_row(this.scan.row);
        this.scan.row = this.composite_row(this.scan.next_chunk_row++);
        scan_next_row(this.scan.cursor, this.scan.row);
      }
    } while (!done && (budget === null || budget.remaining() > 0));
    this.scan.spent_ms += performance.now() - t0;
    if (!done) return "busy";

    this.column_runs = finish_column_scan(this.scan.cursor);
    this.release_row(this.scan.row);
    // Counted as the chunk scans it stands in for, so the ledger's scan rate
    // stays comparable across the unit change: a block scan is one scan
    // per member chunk, done in one place.
    const members = this.unit * this.unit;
    lod_build_stats.scan_ms += this.scan.spent_ms;
    lod_build_stats.scan_count += members;
    lod_build_stats.scan_max_ms = Math.max(lod_build_stats.scan_max_ms, this.scan.spent_ms);
    this.scan = null;
    return "done";
  }

  private advance_bake(step: LodStep, budget: FrameBudget | null): "done" | "busy" | "blocked" {
    const set = this.column_runs!;
    if (this.bake && this.bake.step !== step) {
      abort_bake(this.bake.cursor);
      this.bake = null;
    }
    if (!this.bake) {
      if (bake_open()) return "blocked";
      const cursor = begin_bake(set, step);
      if (cursor === null) return "blocked";
      this.bake = { step, cursor, spent_ms: 0 };
    }
    const t0 = performance.now();
    let done: boolean;
    do {
      done = step_bake(this.bake.cursor);
    } while (!done && (budget === null || budget.remaining() > 0));
    this.bake.spent_ms += performance.now() - t0;
    if (!done) return "busy";

    const { cursor, spent_ms } = this.bake;
    this.bake = null;
    lod_build_stats.build_ms += spent_ms;
    lod_build_stats.build_count += this.unit * this.unit;
    lod_build_stats.build_max_ms = Math.max(lod_build_stats.build_max_ms, spent_ms);
    lod_build_stats.build_runs += set.count;
    // Adopted per region, so the mesh starts entirely hidden: each member
    // reveals its own 64×64 as it adopts. "secondary" is the role whose
    // visibility fill is 0.
    this.baked.set(step, bucket_from_baked(finish_bake(cursor), step, "secondary"));
    return "done";
  }

  /**
   * Drops a scan or bake in flight and keeps every baked tier. What a
   * member calls when it stops driving the block: a baked mesh holds no
   * slot and keeps drawing, but a scan mid-way holds the one scan slot,
   * and a block nobody advances would hold it forever.
   */
  yield_slots(): void {
    this.abort_work();
  }

  private abort_work(): void {
    if (this.scan) {
      abort_column_scan(this.scan.cursor);
      this.release_row(this.scan.row);
      this.scan = null;
    }
    if (this.bake) {
      abort_bake(this.bake.cursor);
      this.bake = null;
    }
  }

  private clear_column_runs(): void {
    if (!this.column_runs) return;
    release_column_runs(this.column_runs);
    this.column_runs = null;
  }
}

/** Every live block, by unit. A member links on its first coarse commit. */
const blocks = new Map<string, ChunkBlock>();

export function block_for(key: ChunkKey, step: LodStep, stack: WorldLayerStack): ChunkBlock {
  const origin = block_key_of(key, step);
  const id = registry_key(origin, block_unit_of(step));
  let block = blocks.get(id);
  if (!block) {
    block = new ChunkBlock(origin, step, stack);
    blocks.set(id, block);
  }
  block.link();
  return block;
}

/** The counterpart of block_for: drops one member's link, and the block
 * itself once no member remains. */
export function block_release(block: ChunkBlock): void {
  if (block.unlink()) blocks.delete(registry_key(block.key, block.unit));
}

export function block_count(): u32 {
  return blocks.size;
}

/**
 * Selects one member's region of a block mesh: visibility `value` on every
 * vertex of every box whose centre lies in the member's 64×64, leaving the
 * rest as they are.
 *
 * The centre rather than the vertex, because a box's far-edge vertices sit
 * exactly on the next region's near edge, and a coarse cell never crosses a
 * chunk edge at these steps (64 is a multiple of 8 and 16), so the centre is
 * strictly inside one region. Each vertex carries its box-local corner in
 * [-0.5, 0.5] and its box's size, so the centre is recovered per vertex
 * without knowing how the mesher grouped them.
 */
export function block_reveal_region(bucket: BuiltChunkBucket, ux: u32, uy: u32, value: f64): void {
  const { position, local_pos, box_scale, visibility, vertex_cnt } = bucket.cubes;
  const x0 = ux * CHUNK_CELL_SPAN, x1 = x0 + CHUNK_CELL_SPAN;
  const y0 = uy * CHUNK_CELL_SPAN, y1 = y0 + CHUNK_CELL_SPAN;
  for (let v = 0; v < vertex_cnt; v++) {
    const i = v * 3;
    const cx = position[i] - local_pos[i] * box_scale[i];
    const cy = position[i + 1] - local_pos[i + 1] * box_scale[i + 1];
    if (cx >= x0 && cx < x1 && cy >= y0 && cy < y1) visibility[v] = value;
  }
}
