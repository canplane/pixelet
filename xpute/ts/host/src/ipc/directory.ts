// @xpute/host/ipc/directory.ts

/**
 * The directory a guest's first turn writes (rust/guest/src/ipc/directory.rs):
 * where the rings and streams are. An unknown key is passed over, so an older
 * host still reads a newer guest.
 */

import type { u32 } from "@xpute/kit/abi/word.ts";
import { Errno } from "@xpute/kit/status/errno.spec.ts";
import { MarshalError } from "@xpute/kit/status/error.ts";
import type { QuantumPolicy } from "../sched/quantum.ts";
import { HEAD, key, MAGIC, QUANTUM_SCALE, VERSION } from "./directory.spec.ts";

export class Directory {
  private readonly entries = new Map<u32, u32>();

  /** A missing or unknown version is refused, and so is a key written twice. */
  constructor(memory: ArrayBufferLike, at: u32) {
    const [magic, version, count] = new Uint32Array(memory, at, HEAD);
    if (magic !== MAGIC) throw new MarshalError(Errno.EBADMSG);
    if (version !== VERSION) throw new MarshalError(Errno.EPROTO);
    const w = new Uint32Array(memory, at + 4 * HEAD, 2 * count);
    for (let i = 0; i < count; i++) {
      if (this.entries.has(w[2 * i])) throw new MarshalError(Errno.EBADMSG);
      this.entries.set(w[2 * i], w[2 * i + 1]);
    }
  }

  /** The key is read as u32: `PROGRAM | n` is negative in JavaScript and would find nothing. */
  get(k: u32): u32 {
    const value = this.entries.get(k >>> 0);
    if (value === undefined) throw new MarshalError(Errno.ENOENT);
    return value;
  }

  quantum(): QuantumPolicy {
    const at = (k: u32) => this.get(k) / QUANTUM_SCALE;
    return { margin_share: at(key.MARGIN_SHARE), batch_frames: at(key.BATCH_FRAMES), settle_ms: at(key.SETTLE_MS) };
  }
}
