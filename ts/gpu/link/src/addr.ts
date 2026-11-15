// @pxl/gpu/link/src/addr.ts

/** An address in the guest's memory as the stream carries it, as
 * pxl-gpu-link `addr.rs` reads it: a u64, the link's address
 * (link/wit/types.wit `lent`), in two words, its low word first (@xpute/kit
 * `split64`), at the places the spec's `OPS` names. */

import type { U32Array } from "@xpute/kit/abi/array.ts";
import { join64, split64, U64, type u64 } from "@xpute/kit/abi/word.ts";
import { OPS } from "./api.spec.ts";

/** Moves every address the records `recs` hold from a block at `from` to
 * the same place in a block at `to`, as `OPS` finds them, the rest
 * untouched. */
export function rebase(recs: U32Array, from: u64, to: u64): void {
  for (let k = 0; k + 2 <= recs.length;) {
    const end = k + 2 + recs[k + 1]!;
    for (const a of OPS) {
      if (a.op !== recs[k]) continue;
      const at = end - a.from_end;
      recs.set(split64(U64(join64(recs[at]!, recs[at + 1]!) - from + to)), at);
    }
    k = end;
  }
}
