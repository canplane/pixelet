// @xpute/runtime/ipc/atomic.ts

/**
 * A word of the core's memory that both ends touch: loaded with acquire,
 * stored with release. A native app's bundler takes atomic.native.ts in its
 * place, for an engine with no `Atomics`.
 */

import type { u32 } from "@xpute/kit/abi/word.ts";

export function load_acquire(w: Uint32Array, i: u32): u32 {
  return Atomics.load(w, i);
}

export function store_release(w: Uint32Array, i: u32, val: u32): void {
  Atomics.store(w, i, val);
}
