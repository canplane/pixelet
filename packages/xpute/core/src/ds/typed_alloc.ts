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
import type { BuddyAllocator } from "./buddy.ts";

export interface TypedAlloc {
  /** A zeroed view of `length` elements. */
  take<T extends AnyTypedArray>(ctor: AnyTypedArrayCtor<T>, length: u32): T;
  /** Hands back a view `take` returned. It must not be used afterwards.
   * Accepts any view: it is only ever looked up, and a caller holding one
   * through a library's broader type (three's attribute arrays) should not
   * need a cast to return it. */
  give(view: ArrayBufferView): void;
}

/**
 * An allocator whose storage can also be named by a number. A lane holds the
 * handle and a view is made from it when needed — so a record is numbers
 * only, and the view it was using can be dropped and made again.
 */
export interface AddressedAlloc extends TypedAlloc {
  /** The handle of the storage a view `take` returned sits over. */
  handle_of(view: ArrayBufferView): u32;
  /** A view of `length` elements over the storage `handle` names. */
  view_of<T extends AnyTypedArray>(ctor: AnyTypedArrayCtor<T>, handle: u32, length: u32): T;
}

export const HEAP_ALLOC: TypedAlloc = {
  take: (ctor, length) => new ctor(length),
  give: () => {},
};

/**
 * Views carved from a buddy pool, one block each.
 *
 * Given back by address rather than by identity: `give` frees whatever block
 * a view sits over, so a view made again from a handle frees the same block
 * the original would have. A view's own byte length is the size it was
 * taken at, which is how the pool recovers the block's order.
 */
export function buddy_alloc(pool: BuddyAllocator): AddressedAlloc {
  return {
    take(ctor, length) {
      const bytes = length * ctor.BYTES_PER_ELEMENT;
      const block = pool.acquire(bytes);
      // A pool block keeps its last borrower's bytes. Zeroed through a byte
      // view because `fill(0)` throws on the BigInt arrays.
      new Uint8Array(block.buf, block.byte_offset, bytes).fill(0);
      return new ctor(block.buf, block.byte_offset, length);
    },
    give(view) {
      pool.release_at(view.buffer as ArrayBuffer, view.byteOffset, view.byteLength);
    },
    handle_of(view) {
      return pool.handle_at(view.buffer as ArrayBuffer, view.byteOffset, view.byteLength);
    },
    view_of(ctor, handle, length) {
      const at = pool.handle_base(handle);
      return new ctor(at.buf, at.byte_offset, length);
    },
  };
}
