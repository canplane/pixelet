// @pxl/web/module/world/change.ts

/**
 * The world's change log: which units of world-shared state changed, read by
 * whoever cares, the way issue/20260913-the-read-direction-a-change-log.md
 * describes. The log is the module's (crates/xpute-core/src/ipc/change.rs), where
 * the state that changes most — the object store — bumps it itself; this is
 * its glue: the journal read as views, and a bump for the sources still
 * here. Topics are numbered here and in crates/pxl-wasm/src/lib.rs's
 * TOPIC_* constants, which the store's own bumps use.
 *
 * A source calls `bump_world`. Readers compare revisions, walk the journal,
 * or subscribe through `world_wakers`, which the tick pumps before its first
 * phase — so a listener runs at the start of the next frame, never inside the
 * write that caused it. The canvas renders on demand, so a bump also asks for
 * that frame; the request is bound by the tick driver, which is the one thing
 * here that knows a canvas exists.
 */

import type { u32 } from "@xpute/core/kit/word.ts";
import type { ChangeJournal, Cursor, Revision, Topic } from "@xpute/core/ipc/change.ts";
import { ChangeWaker } from "@xpute/core/ipc/change_waker.ts";

import type { PxlExports } from "@/wasm/pxl.ts";
import { CORE_PXL, FIXED_ALLOC } from "@/render/layer/pipeline/core_pool.ts";

export const enum WorldTopic {
  /** A column's cube content (object/store.ts), keyed by column base. */
  OBJECT_COLUMN = 0,
  /** A column's placed decor, apart from its cubes, keyed by column base. */
  DECOR_COLUMN = 1,
  /** One cell's placed entries, keyed by cell corner. */
  OBJECT_CELL = 2,
  /** A resident terrain page arrived or left, keyed by page index. */
  TERRAIN_PAGE = 3,
  /** A resident POI page arrived or left, keyed by page index. */
  POI_PAGE = 4,
  /** A decor model or skin finished loading, keyed by its URL's FNV-1a
   * hash. */
  DECOR_ASSET = 5,
}
const WORLD_TOPICS: u32 = 6;

/** The module's log, read in place. Every lane is a view over the module's
 * memory, which never grows, so the views stay good. */
export class ModuleChangeLog implements ChangeJournal {
  readonly topics: u32;
  readonly capacity: u32;
  readonly topic: Uint16Array;
  readonly key: BigInt64Array;
  readonly revision: Uint32Array;
  private readonly head_word: Uint32Array;
  private readonly ticks: Uint32Array;

  constructor(private readonly pxl: PxlExports, topics: u32) {
    const buf = pxl.memory.buffer;
    const row = new Uint32Array(buf, pxl.pxl_change_lanes(), 6);
    this.topics = topics;
    this.capacity = row[5];
    this.topic = new Uint16Array(buf, row[0], this.capacity);
    this.key = new BigInt64Array(buf, row[1], this.capacity);
    this.revision = new Uint32Array(buf, row[2], this.capacity);
    this.head_word = new Uint32Array(buf, row[3], 1);
    this.ticks = new Uint32Array(buf, row[4], topics);
  }

  get head(): Cursor {
    return this.head_word[0];
  }

  readable(cursor: Cursor): boolean {
    return cursor <= this.head && this.head - cursor <= this.capacity;
  }

  slot(position: Cursor): u32 {
    return position % this.capacity;
  }

  bump(topic: Topic, key: bigint): Revision {
    return this.pxl.pxl_change_bump(topic, BigInt.asUintN(64, key));
  }

  forget(topic: Topic, key: bigint): void {
    this.pxl.pxl_change_forget(topic, BigInt.asUintN(64, key));
  }

  revision_of(topic: Topic, key: bigint): Revision {
    return this.pxl.pxl_change_revision(topic, BigInt.asUintN(64, key));
  }

  tick(topic: Topic): Revision {
    return this.ticks[topic];
  }
}

function module_log(): ModuleChangeLog {
  if (CORE_PXL === null) throw new Error("world: the change log is the module's, which needs the core memory");
  return new ModuleChangeLog(CORE_PXL, WORLD_TOPICS);
}

export const world_changes: ModuleChangeLog = module_log();
export const world_wakers: ChangeWaker = new ChangeWaker(world_changes, FIXED_ALLOC);

/** Asks for the frame that delivers changes made in the module — the store's
 * own bumps — the way bump_world does for a change made here. */
export function request_world_tick(): void {
  request_tick();
}

let request_tick: () => void = () => {};

/** The tick driver's frame request. Returns the unbind. */
export function bind_tick_request(request: () => void): () => void {
  request_tick = request;
  return () => {
    if (request_tick === request) request_tick = () => {};
  };
}

/** Records a change to world state and asks for the frame that delivers it. */
export function bump_world(topic: WorldTopic, key: bigint): Revision {
  const revision = world_changes.bump(topic, key);
  request_tick();
  return revision;
}
