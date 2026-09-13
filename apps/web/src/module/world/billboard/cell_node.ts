// @pxl/web/module/world/billboard/cell_node.ts

/**
 * One frame's lattice nodes, and how many things fell into each.
 *
 * The tree, such as it is. A node is a cell of `1 << shamt` canonical units
 * and its address is the candidate's own cell shifted right by that shamt —
 * so every tier exists at once, none of them is stored, and moving between
 * them is an integer shift. That is 3D Tiles' implicit tiling reduced to the
 * part this needs: address by coordinate, build no nodes.
 *
 * It is a hierarchy of aggregates, which puts it in the same family as a
 * segment tree, but not the same shape. A segment tree materialises internal
 * nodes so it can answer an *arbitrary* range; every question asked here is
 * one whole cell at one level, because that is what a badge is. The closer
 * relative is a mipmap: same grid, each level half the resolution, each
 * texel the sum of its children.
 *
 * Shamt is per node rather than per frame, and that is the whole reason for
 * the shape. A perspective camera holds near and far in one view, so one
 * tier for the whole frame is wrong wherever the other end of the view is —
 * the same argument that gives chunks their own LodStep instead of one for
 * the scene.
 *
 * Open addressing over flat integer arrays, generation-stamped rather than
 * cleared, for the same reason spatial_hash.ts is: a per-frame structure
 * that allocates is a per-frame collection pause. There is no Map here and
 * no node object, and the whole thing is five typed arrays and index
 * arithmetic — which is what it will still be under linear memory.
 */

import type { f64, i32, u32 } from "@xpute/core/kit/word.ts";
import { HEAP_ALLOC, type TypedAlloc } from "@xpute/core/ds/typed_alloc.ts";
import { lattice_hash4 } from "@xpute/core/ds/lattice_hash.ts";

/** Marks a slot as never written in this generation. */
const EMPTY: i32 = -1;

export type CellNodeStats = {
  nodes: u32;
  dropped: u32;
  probes: u32;
};

export class CellNodes {
  readonly cap: u32;
  count: u32 = 0;

  /** Cell address at this node's own tier, on each axis. */
  readonly key_x: Int32Array;
  readonly key_y: Int32Array;
  readonly key_z: Int32Array;
  /** The tier, as a shift of the canonical unit. */
  readonly shamt: Uint8Array;

  /** How many candidates fell in. One is a place; more is a count. */
  readonly members: Uint32Array;
  /** The caller's index for the nearest member, which is the one whose
   * identity a single-member node borrows and whose tiebreak orders it. */
  readonly first: Uint32Array;
  /** That member's squared distance, kept so `first` stays the nearest
   * rather than whichever arrived first — arrival order is the query's, and
   * the query's order is the hash's, which is not a fact about the world. */
  readonly first_d2: Float64Array;

  private readonly slots: Int32Array;
  private readonly stamps: Uint32Array;
  private readonly mask: u32;
  private generation: u32 = 0;

  private dropped: u32 = 0;
  private probes: u32 = 0;

  /** `cap` is nodes; the table is the next power of two above twice that, so
   * the load factor stays under a half and probes stay short. */
  constructor(cap: u32, alloc: TypedAlloc = HEAP_ALLOC) {
    this.cap = cap;
    this.key_x = alloc.take(Int32Array, cap);
    this.key_y = alloc.take(Int32Array, cap);
    this.key_z = alloc.take(Int32Array, cap);
    this.shamt = alloc.take(Uint8Array, cap);
    this.members = alloc.take(Uint32Array, cap);
    this.first = alloc.take(Uint32Array, cap);
    this.first_d2 = alloc.take(Float64Array, cap);

    let size = 1;
    while (size < cap * 2) size <<= 1;
    this.slots = alloc.take(Int32Array, size);
    this.stamps = alloc.take(Uint32Array, size);
    this.mask = size - 1;
  }

  /**
   * Opens a generation. Nothing is cleared — a slot whose stamp is stale
   * reads as empty and is overwritten on its next touch.
   */
  begin(): void {
    this.generation = (this.generation + 1) >>> 0;
    if (this.generation === 0) {
      this.stamps.fill(0);
      this.generation = 1;
    }
    this.count = 0;
    this.dropped = 0;
    this.probes = 0;
  }

  /**
   * Files one candidate under its cell, and returns the node it landed in —
   * or EMPTY when there is no room for a new one.
   *
   * `cell_*` is the candidate's address at the *finest* tier; the shift to
   * `shamt` happens here, so a caller never has to hold more than one set of
   * coordinates however many tiers are in play.
   */
  add(cell_x: i32, cell_y: i32, cell_z: i32, shamt: u32, src: u32, d2: f64): i32 {
    const kx = cell_x >> shamt, ky = cell_y >> shamt, kz = cell_z >> shamt;
    // The shamt joins the key: two tiers can be live at once, and a parent's
    // address collides with its child's on a coarse axis.
    const h = lattice_hash4(kx, ky, kz, shamt);

    for (let probe = 0; probe <= this.mask; probe++) {
      const slot = (h + probe) & this.mask;
      this.probes++;

      if (this.stamps[slot] !== this.generation) {
        if (this.count >= this.cap) {
          this.dropped++;
          return EMPTY;
        }
        const id = this.count++;
        this.stamps[slot] = this.generation;
        this.slots[slot] = id;
        this.key_x[id] = kx;
        this.key_y[id] = ky;
        this.key_z[id] = kz;
        this.shamt[id] = shamt;
        this.members[id] = 1;
        this.first[id] = src;
        this.first_d2[id] = d2;
        return id;
      }

      const id = this.slots[slot];
      if (this.key_x[id] !== kx || this.key_y[id] !== ky || this.key_z[id] !== kz || this.shamt[id] !== shamt) continue;

      this.members[id]++;
      if (d2 < this.first_d2[id]) {
        this.first[id] = src;
        this.first_d2[id] = d2;
      }
      return id;
    }
    this.dropped++;
    return EMPTY;
  }

  /** The node's centre, on one axis, in the same units its cells are in —
   * the half being `1 << shamt` over two, so it follows the tier instead of
   * being a constant that is right at one of them. */
  centre(key: i32, shamt: u32): f64 {
    const span = 1 << shamt;
    return key * span + span / 2;
  }

  stats(): CellNodeStats {
    return { nodes: this.count, dropped: this.dropped, probes: this.probes };
  }
}
