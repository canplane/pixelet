// @xpute/runtime/ipc/directory.ts

/**
 * The directory a guest's first turn writes (crates/xpute/runtime/src/ipc/directory.rs),
 * read once by its host: where the rings and streams are, and the policy the
 * guest declares for its turns. The host knows only where the directory is;
 * everything else it reads of the memory, it finds here.
 *
 * An entry is looked up by its key, and one the host does not know is passed
 * over, so a guest that adds an entry is still read by a host that predates
 * it.
 */

import type { u32 } from "@xpute/core/abi/word.ts";
import { Errno } from "@xpute/core/status/errno.spec.ts";
import { MarshalError } from "@xpute/core/status/error.ts";
import type { QuantumPolicy } from "../sched/quantum.ts";
import { HEAD, key, MAGIC, QUANTUM_SCALE, VERSION } from "./directory.spec.ts";

export class Directory {
  private readonly entries = new Map<u32, u32>();

  /** The directory at `at`. A head that is not one — memory the guest has
   * not written, or a version this host does not read — is refused, and so is
   * a key written twice, which a reader would have to pick a value for. */
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

  /** The value under `k`; a key the guest did not write is refused, since
   * what the host asks for here it cannot run without. The key is taken as
   * the u32 it is on the wire: one made with `PROGRAM | n` is a negative
   * number in JavaScript, and would otherwise find nothing. */
  get(k: u32): u32 {
    const value = this.entries.get(k >>> 0);
    if (value === undefined) throw new MarshalError(Errno.ENOENT);
    return value;
  }

  /** The quantum policy the guest declared. */
  quantum(): QuantumPolicy {
    const at = (k: u32) => this.get(k) / QUANTUM_SCALE;
    return { margin_share: at(key.MARGIN_SHARE), batch_frames: at(key.BATCH_FRAMES), settle_ms: at(key.SETTLE_MS) };
  }
}
