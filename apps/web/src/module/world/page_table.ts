// @pxl/web/module/world/page_table.ts

/**
 * The pages resident in the client, keyed by L3 page index, as a table.
 *
 * Terrain and POI each kept their pages in a `Map<IVec, …>` inside a
 * zustand store, and every reader either looked a page up or walked the
 * map. The key index is now an IndexedMap in the core memory — dense
 * positions, an open-addressing table over packed page keys — and what a
 * page holds sits at its position in a handle array: for terrain the typed
 * views over the page's pool blocks, for POI the page's records. The
 * handles are the glue three.js and the mesher read through; the keys, the
 * positions and the walk are linear.
 *
 * A table given a world topic writes every set and clear to the world
 * change log (module/world/change.ts) — which page, as its own key — and its
 * `revision` is that topic's tick; readers compare it, or hear the pages
 * through the log's wakers. Without a topic it only counts. The keys are normalised through TO_XY on the way in
 * (xy.ts on why), and come back out unsigned, the form every IVec is.
 */

import type { u32 } from "@xpute/core/kit/word.ts";
import { I64_KEYS, IndexedMap } from "@xpute/core/ds/map.ts";

import type { IVec } from "@pxl/shared/spec/space/pvf3/mod.ts";
import { TO_XY } from "@pxl/shared/spec/space/xy.ts";

import { CORE_ALLOC } from "@/render/layer/pipeline/core_pool.ts";
import { bump_world, world_changes, type WorldTopic } from "@/module/world/change.ts";

export class PageTable<T> {
  private readonly index = new IndexedMap<bigint>(I64_KEYS, 64, CORE_ALLOC);
  /** What each position holds. */
  readonly handles: T[] = [];
  private count: u32 = 0;

  constructor(readonly topic: WorldTopic | null = null) {}

  /** Moves on every set and clear: the topic's tick, or a count. */
  get revision(): u32 {
    return this.topic === null ? this.count : world_changes.tick(this.topic);
  }

  get size(): u32 {
    return this.index.size;
  }

  get(page_idx: IVec): T | undefined {
    const pos = this.index.find(TO_XY(page_idx));
    return pos < 0 ? undefined : this.handles[pos];
  }

  has(page_idx: IVec): boolean {
    return this.index.find(TO_XY(page_idx)) >= 0;
  }

  set(page_idx: IVec, value: T): void {
    const key = TO_XY(page_idx);
    const pos = this.index.insert(key);
    this.handles[pos] = value;
    this.changed(key, false);
  }

  /** Removes the page and returns what it held, or undefined. */
  clear(page_idx: IVec): T | undefined {
    const key = TO_XY(page_idx);
    const pos = this.index.find(key);
    if (pos < 0) return undefined;
    const held = this.handles[pos];
    const from = this.index.remove_at(pos);
    if (from !== pos) this.handles[pos] = this.handles[from];
    this.handles.length = from;
    this.changed(key, true);
    return held;
  }

  /** The page key at a position, as the unsigned IVec it was set with. */
  key_at(pos: u32): IVec {
    return BigInt.asUintN(64, this.index.key_at(pos));
  }

  *keys(): IterableIterator<IVec> {
    for (let pos = 0; pos < this.index.size; pos++) yield this.key_at(pos);
  }

  *entries(): IterableIterator<[IVec, T]> {
    for (let pos = 0; pos < this.index.size; pos++) yield [this.key_at(pos), this.handles[pos]];
  }

  private changed(key: IVec, left: boolean): void {
    this.count++;
    if (this.topic === null) return;
    const unsigned = BigInt.asUintN(64, key);
    bump_world(this.topic, unsigned);
    if (left) world_changes.forget(this.topic, unsigned);
  }
}
