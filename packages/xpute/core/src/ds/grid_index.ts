// @xpute/core/ds/grid_index.ts

/**
 * Screen-space collision grid for greedy label placement.
 *
 * The question a spatial hash cannot answer. `spatial_hash.ts` indexes where
 * things are in the world; this indexes where they have already been *drawn*,
 * so the next candidate can find out whether the pixels it wants are free.
 * A renderer with more labels than room needs both, and they are different
 * structures because they are different spaces — one unbounded and
 * three-dimensional, one exactly the size of the viewport and flat.
 *
 * Mapbox GL's `GridIndex` is the shape, and the reason to follow it rather
 * than invent is that the failure modes here are not obvious: labels that
 * flicker as the camera moves, labels that vanish when a more important one
 * appears two cells away, ties that resolve differently frame to frame.
 * Theirs divides a 600x600 viewport into 30 px cells — 400 of them — and
 * reports near-constant query cost, since "doubling the dimensions of the
 * index is not expected to have a significant impact on the cost of a
 * query".
 *
 * **Greedy, in the caller's order, and that order is the design.** For each
 * candidate: test, and place if the space is free. There is no global
 * optimisation and no backtracking — first fit wins, so whatever sequence
 * the caller supplies *is* the priority rule. A candidate that loses is not
 * retried at a different position; a renderer that wants alternate anchors
 * offers them as separate candidates.
 *
 * **Placement is not a per-frame job.** Mapbox runs this synchronously but
 * explicitly not every frame, on the grounds that "a latency of a couple
 * hundred milliseconds in actually detecting a new collision is perceptually
 * acceptable". The structure is cheap enough either way; the caller decides.
 *
 * **Everything is preallocated and a rebuild allocates nothing.** Cells hold
 * an intrusive singly-linked list over one node pool, and a generation stamp
 * makes a stale cell read as empty instead of being cleared — the same two
 * devices `spatial_hash.ts` uses, for the same reason. Running out of nodes
 * is reported rather than absorbed: a full grid means every later candidate
 * collides, which is the honest answer and also the one that keeps the live
 * object count fixed.
 */

import type { f64, i32, u32 } from "../kit/word.ts";

/** Mapbox's, on their reference viewport. Small enough that a label spans a
 * handful of cells, large enough that the grid stays short. */
export const DEFAULT_CELL_PX = 30;

export type GridIndexStats = {
  /** Rectangles placed in the current generation. */
  placed: u32;
  /** Placements refused for want of a rectangle slot or a node. */
  dropped: u32;
  /** Node-pool entries consumed. */
  nodes: u32;
};

/** `place` / `first_hit`: the rectangle owns its space, nothing blocked it. */
export const PLACED: i32 = -1;
/** `place`: refused for want of a slot or a node, so it is neither drawn nor
 * attributable to anything. Reported through `stats().dropped` too. */
export const NO_ROOM: i32 = -2;

export class GridIndex {
  private readonly cell_px: f64;
  private readonly capacity: u32;
  private readonly node_capacity: u32;

  private cols: u32 = 0;
  private rows: u32 = 0;

  /** x0, y0, x1, y1 interleaved, so one rectangle is one cache line's worth
   * of contiguous reads rather than four strided ones. */
  private readonly rects: Float32Array;
  private placed: u32 = 0;
  private dropped: u32 = 0;

  /** Head node index per cell, valid only while the stamp is current. */
  private readonly heads: Int32Array;
  private readonly stamps: Uint32Array;
  private generation: u32 = 0;

  /** Intrusive list: `node_rect[n]` is in this cell, `node_next[n]` is the
   * next one, -1 ending the chain. */
  private readonly node_rect: Uint32Array;
  private readonly node_next: Int32Array;
  private nodes: u32 = 0;

  /**
   * @param max_width_px   largest viewport this will be asked about
   * @param max_height_px  same
   * @param capacity       most rectangles placed in one generation
   * @param cell_px        cell edge; a label should span only a few
   * @param nodes_per_rect budget for how many cells one rectangle covers —
   *                       exceeding it stops placement rather than growing
   */
  constructor(
    max_width_px: u32,
    max_height_px: u32,
    capacity: u32,
    cell_px: f64 = DEFAULT_CELL_PX,
    nodes_per_rect: u32 = 8,
  ) {
    this.cell_px = cell_px;
    this.capacity = capacity;
    this.node_capacity = capacity * nodes_per_rect;

    const max_cells = Math.ceil(max_width_px / cell_px + 1) * Math.ceil(max_height_px / cell_px + 1);
    this.heads = new Int32Array(max_cells);
    this.stamps = new Uint32Array(max_cells);

    this.rects = new Float32Array(capacity * 4);
    this.node_rect = new Uint32Array(this.node_capacity);
    this.node_next = new Int32Array(this.node_capacity);
  }

  /**
   * Opens a generation for a viewport of this size. Nothing is cleared: a
   * cell whose stamp is stale reads as empty on its next touch.
   *
   * The viewport is taken per generation rather than per construction
   * because a window resize must not reallocate — the grid was sized for the
   * largest case at construction and simply uses fewer cells when smaller.
   */
  begin(width_px: f64, height_px: f64): void {
    this.cols = Math.max(1, Math.ceil(width_px / this.cell_px));
    this.rows = Math.max(1, Math.ceil(height_px / this.cell_px));

    this.generation = (this.generation + 1) >>> 0;
    if (this.generation === 0) {
      this.stamps.fill(0);
      this.generation = 1;
    }
    this.placed = 0;
    this.dropped = 0;
    this.nodes = 0;
  }

  private col_of(x: f64): i32 {
    return Math.min(this.cols - 1, Math.max(0, Math.floor(x / this.cell_px)));
  }
  private row_of(y: f64): i32 {
    return Math.min(this.rows - 1, Math.max(0, Math.floor(y / this.cell_px)));
  }

  /**
   * Whether this rectangle overlaps anything already placed.
   *
   * A rectangle entirely off-screen collides with nothing, which is correct
   * rather than convenient: this structure only knows about pixels, and
   * deciding that something out of view should not be drawn belongs to
   * whatever culls before this runs.
   */
  hit_test(x0: f64, y0: f64, x1: f64, y1: f64): boolean {
    return this.first_hit(x0, y0, x1, y1) !== PLACED;
  }

  /**
   * Which placed rectangle this one runs into first, or PLACED for none.
   *
   * The id is the caller's, in the sense that `insert` hands them out in
   * order — so a caller placing candidates one at a time reads it back as
   * "the candidate that took this space". That is what turns a rejection
   * into an attribution: a label that cannot be drawn is not nothing, it is
   * one more thing standing where the winner stands, and only the structure
   * that refused it knows which winner that was.
   *
   * "First" is the first found, not the nearest — with the caller placing in
   * priority order every occupant of a contested space is one this candidate
   * already lost to, so any of them is an honest answer.
   */
  first_hit(x0: f64, y0: f64, x1: f64, y1: f64): i32 {
    if (x1 < 0 || y1 < 0 || x0 > this.cols * this.cell_px || y0 > this.rows * this.cell_px) return PLACED;

    const c0 = this.col_of(x0), c1 = this.col_of(x1);
    const r0 = this.row_of(y0), r1 = this.row_of(y1);

    for (let r = r0; r <= r1; r++) {
      for (let c = c0; c <= c1; c++) {
        const cell = r * this.cols + c;
        if (this.stamps[cell] !== this.generation) continue;
        for (let n = this.heads[cell]; n !== -1; n = this.node_next[n]) {
          const o = this.node_rect[n] * 4;
          // A rectangle in more than one shared cell is reached more than
          // once. Re-testing it is cheaper than the bookkeeping that would
          // avoid it, since the first overlap returns immediately anyway.
          if (x0 <= this.rects[o + 2] && x1 >= this.rects[o] && y0 <= this.rects[o + 3] && y1 >= this.rects[o + 1]) {
            return this.node_rect[n];
          }
        }
      }
    }
    return PLACED;
  }

  /**
   * Records a rectangle as occupied without testing it — for the things that
   * are drawn whatever else wants the space, and which later candidates must
   * still avoid.
   *
   * Returns false when a slot or a node is unavailable, which leaves the
   * rectangle unregistered: from then on the grid under-reports occupancy,
   * so a caller that cares reads `stats().dropped` rather than assuming.
   */
  insert(x0: f64, y0: f64, x1: f64, y1: f64): boolean {
    if (this.placed >= this.capacity) {
      this.dropped++;
      return false;
    }
    const c0 = this.col_of(x0), c1 = this.col_of(x1);
    const r0 = this.row_of(y0), r1 = this.row_of(y1);

    const needed = (c1 - c0 + 1) * (r1 - r0 + 1);
    if (this.nodes + needed > this.node_capacity) {
      this.dropped++;
      return false;
    }

    const id = this.placed++;
    const o = id * 4;
    this.rects[o] = x0;
    this.rects[o + 1] = y0;
    this.rects[o + 2] = x1;
    this.rects[o + 3] = y1;

    for (let r = r0; r <= r1; r++) {
      for (let c = c0; c <= c1; c++) {
        const cell = r * this.cols + c;
        if (this.stamps[cell] !== this.generation) {
          this.stamps[cell] = this.generation;
          this.heads[cell] = -1;
        }
        const n = this.nodes++;
        this.node_rect[n] = id;
        this.node_next[n] = this.heads[cell];
        this.heads[cell] = n;
      }
    }
    return true;
  }

  /**
   * The greedy step: take the space if it is free, and otherwise say who has
   * it — PLACED when this rectangle now occupies it, NO_ROOM when the
   * structure is full, and any other value being the id that blocked it.
   *
   * Test and insert are separately available because some callers need one
   * without the other, but this pairing is the operation the structure
   * exists for, and keeping it here means the two halves cannot drift apart
   * — a caller that tested one rectangle and inserted a slightly different
   * one would corrupt every later answer.
   */
  place(x0: f64, y0: f64, x1: f64, y1: f64): i32 {
    const blocker = this.first_hit(x0, y0, x1, y1);
    if (blocker !== PLACED) return blocker;
    return this.insert(x0, y0, x1, y1) ? PLACED : NO_ROOM;
  }

  stats(): GridIndexStats {
    return { placed: this.placed, dropped: this.dropped, nodes: this.nodes };
  }
}
