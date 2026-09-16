// @xpute/core/ds/typed_alloc.ts

/**
 * Where a typed lane's storage comes from.
 *
 * Every lane in the core — an index's columns, a batch's scratch, a mesh's
 * instance matrices — is a typed array, and each one used to be
 * `new Ctor(n)`: a buffer of its own on the JS heap. The core's data is meant
 * to live in one linear memory instead, which is the one WASM will address,
 * so structures here take their storage from an allocator rather than
 * constructing it.
 *
 * `take` keeps `new Ctor(n)`'s contract exactly — a zeroed view of `n`
 * elements — so a structure switches allocators without its own code
 * changing meaning. `HEAP_ALLOC` is `new Ctor(n)` itself and the default
 * everywhere, so nothing moves until a caller hands in another.
 */

import type { u32 } from "../kit/word.ts";
import type { AnyTypedArray, AnyTypedArrayCtor } from "../kit/array.ts";

export interface TypedAlloc {
  /** A zeroed view of `length` elements. */
  take<T extends AnyTypedArray>(ctor: AnyTypedArrayCtor<T>, length: u32): T;
  /** Hands back a view `take` returned. It must not be used afterwards.
   * Accepts any view: it is only ever looked up, and a caller holding one
   * through a library's broader type (three's attribute arrays) should not
   * need a cast to return it. */
  give(view: ArrayBufferView): void;
}

export const HEAP_ALLOC: TypedAlloc = {
  take: (ctor, length) => new ctor(length),
  give: () => {},
};

/**
 * Consecutive typed views over one block, cut in call order from
 * `byte_offset` on — the shape every "several spans, one allocation"
 * structure carves by hand. Views are laid where the previous one ended,
 * so a caller orders them widest element first (i32 before u8) to keep
 * each aligned to its element; the block's own start is 8-aligned.
 */
export function carver(buf: ArrayBuffer, byte_offset: u32): <T extends AnyTypedArray>(ctor: AnyTypedArrayCtor<T>, length: u32) => T {
  let off = byte_offset;
  return (ctor, length) => {
    const view = new ctor(buf, off, length);
    off += length * ctor.BYTES_PER_ELEMENT;
    return view;
  };
}
