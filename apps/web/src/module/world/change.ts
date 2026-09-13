// @pxl/web/module/world/change.ts

/**
 * The world's change log: which units of world-shared state changed, read by
 * whoever cares, the way issue/20260913-the-read-direction-a-change-log.md
 * describes. Topics are numbered here and nowhere else.
 *
 * A source calls `bump_world`. Readers compare revisions, walk the journal,
 * or subscribe through `world_wakers`, which the tick pumps before its first
 * phase — so a listener runs at the start of the next frame, never inside the
 * write that caused it. The canvas renders on demand, so a bump also asks for
 * that frame; the request is bound by the tick driver, which is the one thing
 * here that knows a canvas exists.
 */

import type { u32 } from "@xpute/core/kit/word.ts";
import { ChangeLog, type Revision } from "@xpute/core/ipc/change.ts";
import { ChangeWaker } from "@xpute/core/ipc/change_waker.ts";

import { CORE_ALLOC } from "@/render/layer/pipeline/core_pool.ts";

export const enum WorldTopic {
  /** A column's cube content (object/store.ts), keyed by column base. */
  OBJECT_COLUMN = 0,
  /** A column's placed decor, apart from its cubes, keyed by column base. */
  DECOR_COLUMN = 1,
  /** One cell's placed entries, keyed by cell corner. */
  OBJECT_CELL = 2,
}
const WORLD_TOPICS: u32 = 3;

/** Enough for a zoom-out's worth of pages and a burst of edits between two
 * frames; a reader further behind re-reads its topics. */
const JOURNAL_CAPACITY: u32 = 4096;

export const world_changes: ChangeLog = new ChangeLog({ topics: WORLD_TOPICS, capacity: JOURNAL_CAPACITY, keys_per_topic: 256, alloc: CORE_ALLOC });
export const world_wakers: ChangeWaker = new ChangeWaker(world_changes, CORE_ALLOC);

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
