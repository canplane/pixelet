// @xpute/runtime/ipc/change_waker.ts

/**
 * Push delivery over a change log: listeners per (topic, key), woken when a
 * pump finds their key in the journal.
 *
 * An adapter, not the protocol. The log never calls anyone; whoever owns the
 * pump decides when listeners run — once a tick, say — so a burst of changes
 * to one key wakes its listeners once, and a source's write never runs
 * consumer code inside it. A pump that finds its cursor overwritten wakes
 * every listener it holds: each of them re-reads, which is what `lost`
 * means.
 *
 * A topic listener hears every changed key of its topic instead, once per
 * key per pump, and null when the pump was lost — its filter is its own.
 */

import type { u32, u64 } from "@xpute/core/abi/word.ts";
import type { U32Array } from "@xpute/core/abi/array.ts";
import { Vector } from "@xpute/core/collection/arena.ts";
import { I64_KEYS, IndexedMap } from "@xpute/core/collection/map.ts";

import type { ChangeJournal, Cursor, Topic } from "./change.ts";

type Listener = () => void;
/** A changed key, or null: the pump lost its place, re-read the topic. */
export type TopicListener = (key: u64 | null) => void;

export class ChangeWaker {
  private cursor: Cursor;
  private pumps: u32 = 0;
  /** Per topic: the keys listened to, their listeners, and the pump that
   * last woke each, so a key bumped twice in one span wakes once. */
  private readonly keys: IndexedMap<u64>[] = [];
  private readonly lists: Listener[][][] = [];
  private readonly woken: Vector<U32Array>[] = [];
  /** Per topic: whole-topic listeners, and the keys already given to them
   * this pump. */
  private readonly topic_lists: TopicListener[][] = [];
  private readonly topic_seen: IndexedMap<u64>[] = [];

  constructor(private readonly log: ChangeJournal) {
    this.cursor = log.head;
    for (let t = 0; t < log.topics; t++) {
      this.keys.push(new IndexedMap(I64_KEYS, 16));
      this.lists.push([]);
      this.woken.push(new Vector(Uint32Array, { init_cap: 16 }));
      this.topic_lists.push([]);
      this.topic_seen.push(new IndexedMap(I64_KEYS, 64));
    }
  }

  subscribe(topic: Topic, key: u64, listener: Listener): () => void {
    const keys = this.keys[topic], lists = this.lists[topic], woken = this.woken[topic];
    const pos = keys.insert(key);
    if (pos === woken.len) woken.push(0);
    (lists[pos] ??= []).push(listener);
    return () => {
      const p = keys.find(key);
      if (p < 0) return;
      const list = lists[p];
      const at = list.indexOf(listener);
      if (at >= 0) list.splice(at, 1);
      if (list.length > 0) return;
      const from = keys.remove_at(p);
      if (from !== p) {
        lists[p] = lists[from];
        woken.set(p, woken.get(from));
      }
      lists.length = from;
      woken.truncate(from);
    };
  }

  /** Hears every key of `topic` that changes. */
  subscribe_topic(topic: Topic, listener: TopicListener): () => void {
    const list = this.topic_lists[topic];
    list.push(listener);
    return () => {
      const at = list.indexOf(listener);
      if (at >= 0) list.splice(at, 1);
    };
  }

  /** Wakes the listeners of every key changed since the last pump. Returns
   * how many listeners ran. */
  pump(): u32 {
    const log = this.log;
    if (this.cursor === log.head) return 0;
    const pump = ++this.pumps;
    let ran = 0;
    if (!log.readable(this.cursor)) {
      for (let t = 0; t < this.lists.length; t++) {
        for (let p = 0; p < this.keys[t].size; p++) ran += this.wake(t, p, pump);
        for (const listener of this.topic_lists[t].slice()) listener(null), ran++;
      }
    } else {
      for (const seen of this.topic_seen) seen.clear();
      for (let c = this.cursor; c < log.head; c++) {
        const i = log.slot(c);
        const t = log.topic[i];
        const key = log.key[i];
        const p = this.keys[t].find(key);
        if (p >= 0) ran += this.wake(t, p, pump);
        const topic_list = this.topic_lists[t];
        if (topic_list.length === 0) continue;
        const seen = this.topic_seen[t];
        if (seen.find(key) >= 0) continue;
        seen.insert(key);
        for (const listener of topic_list.slice()) listener(key), ran++;
      }
    }
    this.cursor = log.head;
    return ran;
  }

  private wake(topic: Topic, pos: u32, pump: u32): u32 {
    const woken = this.woken[topic];
    if (woken.get(pos) === pump) return 0;
    woken.set(pos, pump);
    // A listener may unsubscribe as it runs; walk what was there when the
    // wake began.
    const list = this.lists[topic][pos].slice();
    for (const listener of list) listener();
    return list.length;
  }
}
