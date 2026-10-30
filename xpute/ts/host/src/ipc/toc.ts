// @xpute/host/ipc/toc.ts

/**
 * The toc a guest's first turn writes (rust/guest/src/ipc/toc.rs):
 * where the rings and streams are. An unknown key is passed over, so an older
 * host still reads a newer guest.
 */

import type { u32 } from "@xpute/kit/abi/word.ts";
import * as Errno from "@xpute/kit/status/errno.spec.ts";
import { MarshalError } from "@xpute/kit/status/error.ts";
import { HEAD, MAGIC, VERSION } from "./toc.spec.ts";

export class Toc {
  private readonly entries = new Map<u32, u32>();

  /** A missing or unknown version is refused, and so is a key written twice. */
  constructor(mem: ArrayBufferLike, at: u32) {
    const [magic, version, cnt] = new Uint32Array(mem, at, HEAD);
    if (magic !== MAGIC) throw new MarshalError(Errno.EBADMSG);
    if (version !== VERSION) throw new MarshalError(Errno.EPROTO);
    const w = new Uint32Array(mem, at + 4 * HEAD, 2 * cnt);
    for (let i = 0; i < cnt; i++) {
      if (this.entries.has(w[2 * i])) throw new MarshalError(Errno.EBADMSG);
      this.entries.set(w[2 * i], w[2 * i + 1]);
    }
  }

  /** The key is read as u32: `PROGRAM | n` is negative in JavaScript and would find nothing. */
  get(k: u32): u32 {
    const val = this.entries.get(k >>> 0);
    if (val === undefined) throw new MarshalError(Errno.ENOENT);
    return val;
  }
}
