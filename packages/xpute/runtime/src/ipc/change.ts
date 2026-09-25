// @xpute/runtime/ipc/change.ts

/**
 * The read half of the channel: which units of state changed, as data.
 *
 * A change is three numbers — a topic (u16, a kind of state the caller
 * numbers), a key (u64, the unit within it) and a revision (u32). A source
 * calls `bump`; a reader either compares a key's revision with the one it
 * remembers, or reads the journal from its own cursor. How a reader is woken
 * — polled from a tick, a listener, a React hook, a worker's mirror — is
 * built over this and is not part of it
 * (issue/20260913-the-read-direction-a-change-log.md).
 *
 * A revision is the topic's tick at the key's last change, one counter per
 * topic rather than per key, so a key forgotten and changed again gets a
 * revision it never had and a reader holding an old one cannot mistake the
 * new state for the one it saw — Bevy's change ticks, per topic. The tick is
 * u32 and a wrap is not handled: four billion changes to one topic.
 *
 * The journal is one fixed-capacity ring for every topic, laid out as lanes,
 * indexed by position modulo capacity. A cursor is a position; positions only
 * grow, and a cursor more than `capacity` behind the head has been
 * overwritten and is not readable — the reader re-reads the topics it cares
 * about and takes the head. Nothing queues per reader, so the log's memory is
 * fixed however many read it.
 *
 * A change carries no value. The value is the owner's, read by key where it
 * lives, so the journal stays small and a span of it crosses a boundary as a
 * frame body unchanged.
 *
 * Keys are BigInt here, the form the rest of the core keys 64-bit units in;
 * reading one back from its lane makes a BigInt, the cost a Rust core does
 * not pay. The layout is what matters now.
 */

import type { u32 } from "@xpute/core/abi/word.ts";
import type { I64Array, U32Array } from "@xpute/core/abi/array.ts";
import { Vector } from "@xpute/core/collection/arena.ts";
import { I64_KEYS, IndexedMap } from "@xpute/core/collection/map.ts";
import { Errno } from "@xpute/core/status/errno.spec.ts";
import { InvariantError } from "@xpute/core/status/error.ts";

export type Topic = u32;
export type Revision = u32;
/** A position in the journal: how many changes had been written before it. */
export type Cursor = number;

export interface ChangeLogOptions {
  /** Topics are 0 .. topics - 1. */
  topics: u32;
  /** Journal entries kept; a reader further behind than this is lost. */
  capacity: u32;
  /** Keys a topic's table holds before it grows. */
  keys_per_topic?: u32;
}

/** What a reader walks: the journal's lanes and positions. ChangeLog is one;
 * a log kept in a WASM core and read through views is another. */
export interface ChangeJournal {
  readonly topics: u32;
  readonly head: Cursor;
  readable(cursor: Cursor): boolean;
  slot(position: Cursor): u32;
  readonly topic: Uint16Array<ArrayBufferLike>;
  readonly key: BigInt64Array<ArrayBufferLike>;
  readonly revision: Uint32Array<ArrayBufferLike>;
}

export class ChangeLog implements ChangeJournal {
  readonly capacity: u32;

  /** Journal lanes. Entry at position p is at index `slot(p)`. */
  readonly topic: Uint16Array;
  readonly key: I64Array;
  readonly revision: U32Array;

  private _head: Cursor = 0;
  private readonly ticks: U32Array;
  private readonly tables: IndexedMap<bigint>[] = [];
  private readonly revisions: Vector<U32Array>[] = [];

  constructor(opts: ChangeLogOptions) {
    const { topics, capacity, keys_per_topic = 64 } = opts;
    if (topics < 1 || topics > 0x10000) throw new InvariantError(Errno.EINVAL);
    if (capacity < 1) throw new InvariantError(Errno.EINVAL);
    this.capacity = capacity;
    this.topic = new Uint16Array(capacity);
    this.key = new BigInt64Array(capacity);
    this.revision = new Uint32Array(capacity);
    this.ticks = new Uint32Array(topics);
    for (let t = 0; t < topics; t++) {
      this.tables.push(new IndexedMap(I64_KEYS, keys_per_topic));
      this.revisions.push(new Vector(Uint32Array, { init_cap: keys_per_topic }));
    }
  }

  get topics(): u32 {
    return this.ticks.length;
  }

  // ---- source ----

  /** Records that `key` in `topic` changed. Returns its new revision. */
  bump(topic: Topic, key: bigint): Revision {
    const tick = (this.ticks[topic] + 1) >>> 0;
    this.ticks[topic] = tick;

    const pos = this.tables[topic].insert(key);
    const revs = this.revisions[topic];
    if (pos === revs.len) revs.push(tick);
    else revs.set(pos, tick);

    const i = this.slot(this._head);
    this.topic[i] = topic;
    this.key[i] = BigInt.asIntN(64, key);
    this.revision[i] = tick;
    this._head++;
    return tick;
  }

  /** Drops a key that has left — a page unloaded — from its topic's table.
   * Its revision reads 0 afterward; the journal keeps what was written. */
  forget(topic: Topic, key: bigint): void {
    const table = this.tables[topic];
    const pos = table.find(key);
    if (pos < 0) return;
    const from = table.remove_at(pos);
    const revs = this.revisions[topic];
    if (from !== pos) revs.set(pos, revs.get(from));
    revs.truncate(from);
  }

  // ---- reader: point ----

  /** The key's revision, or 0 when it has never changed or was forgotten. */
  revision_of(topic: Topic, key: bigint): Revision {
    const pos = this.tables[topic].find(key);
    return pos < 0 ? 0 : this.revisions[topic].get(pos);
  }

  /** The topic's tick: moved means something in the topic changed. */
  tick(topic: Topic): Revision {
    return this.ticks[topic];
  }

  // ---- reader: journal ----

  /** The position the next change will be written at. A reader that has
   * read up to here takes it as its cursor. */
  get head(): Cursor {
    return this._head;
  }

  /** Whether every entry from `cursor` to the head is still in the ring. */
  readable(cursor: Cursor): boolean {
    return cursor <= this._head && this._head - cursor <= this.capacity;
  }

  /** Where the entry at a position lives in the lanes. */
  slot(position: Cursor): u32 {
    return position % this.capacity;
  }
}
