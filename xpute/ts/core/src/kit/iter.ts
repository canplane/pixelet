// @xpute/core/kit/iter.ts

import type { u32 } from "../abi/word.ts";
export function* dedupe<T>(it: Iterable<T>): IterableIterator<T> {
  const seen = new Set<T>();
  for (const e of it) {
    if (seen.has(e)) continue;
    seen.add(e);
    yield e;
  }
}

export function* except<T>(universe: Iterable<T>, excluded: Iterable<T>): IterableIterator<T> {
  const ban = new Set<T>();
  for (const e of excluded) ban.add(e);

  for (const e of universe) {
    if (ban.has(e)) continue;
    yield e;
  }
}

export function* take<T>(it: Iterable<T>, k: u32): IterableIterator<T> {
  if (k <= 0) return;
  let i = 0;
  for (const e of it) {
    yield e;
    if (++i >= k) break;
  }
}

export function* filter<T>(it: Iterable<T>, pred: (e: T) => boolean): IterableIterator<T> {
  for (const e of it) if (pred(e)) yield e;
}
