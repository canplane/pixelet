// @xpute/runtime/ipc/atomic.native.ts

/**
 * A word of the core's memory that both ends touch, where the JavaScript
 * engine has no `Atomics` (Hermes) and the core runs on a thread of its own:
 * the load and the store are made in native code, as the core makes its own
 * (xpute-runtime `Core::load`, `Core::store`). `w` is a view of `XPUTE.mem()`,
 * so its byte offset is the word's place in the memory.
 */

import type { u32 } from "@xpute/kit/abi/word.ts";
import { XPUTE } from "@xpute/native";

export function load_acquire(w: Uint32Array, i: u32): u32 {
  return XPUTE.load(w.byteOffset + 4 * i);
}

export function store_release(w: Uint32Array, i: u32, val: u32): void {
  XPUTE.store(w.byteOffset + 4 * i, val);
}
