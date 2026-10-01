// @xpute/runtime/ipc/sys.ts

/**
 * The host's end of a guest's system calls (crates/xpute/runtime/src/ipc/
 * sys.rs, sys.spec.ts): the records a turn left in the stream the directory
 * names under SYS, run once the turn has returned and before the next begins,
 * so whatever one names in the guest's memory is written by the time the
 * guest reads it.
 *
 * Every program's calls are run here: a body read out of what the host holds
 * for the guest, a file written, a broken invariant reported. Where the bytes
 * go when written, and how a report is said, is the program's host to answer;
 * a call numbered with the PROGRAM bit is handed to it whole.
 */

import type { U32Array, U8Array } from "@xpute/core/abi/array.ts";
import type { u32 } from "@xpute/core/abi/word.ts";
import { Errno } from "@xpute/core/status/errno.spec.ts";
import { InvariantError } from "@xpute/core/status/error.ts";
import { bytes_of, each_record } from "./stream.ts";
import { HEAD, op } from "./sys.spec.ts";

/** Bytes the host holds for the guest — a response, a file, what storage
 * kept — by a number a command hands over, until the guest reads them. */
export class Bodies {
  private readonly held = new Map<u32, U8Array>();
  private next: u32 = 1;

  /** Holds `bytes`: the number a command names them by. */
  hold(bytes: U8Array): u32 {
    const body = this.next;
    this.next = (this.next + 1) >>> 0 || 1;
    this.held.set(body, bytes);
    return body;
  }

  /** The bytes held as `body`, let go. */
  take(body: u32): U8Array | undefined {
    const bytes = this.held.get(body);
    this.held.delete(body);
    return bytes;
  }
}

/** A broken invariant the guest reported before it stopped. */
export interface Abort {
  errno: number;
  file: string;
  line: u32;
  column: u32;
  a: bigint;
  b: bigint;
}

/** What only the program's host can answer. */
export interface SysHost {
  /** `bytes` written whole to the program's file `file`. */
  write(file: u32, bytes: U8Array): void;
  /** What stopped the guest. */
  abort(report: Abort): void;
  /** A call of the program's own: its number, the record's words, and where
   * its arguments begin in them. */
  program(code: u32, w: U32Array, a: u32): void;
}

const utf8 = new TextDecoder();

/** Runs the calls the turn just returned left in the stream at `at`. */
export function run_sys(memory: ArrayBuffer, at: u32, bodies: Bodies, host: SysHost): void {
  each_record(memory, at, HEAD, (code, w, a) => {
    switch (code) {
      case op.READ: {
        const [body, dst, len] = [w[a], w[a + 1], w[a + 2]];
        const bytes = bodies.take(body);
        if (bytes !== undefined && len > 0) new Uint8Array(memory, dst, len).set(bytes.subarray(0, len));
        break;
      }
      case op.WRITE:
        // A copy: the guest keeps the range only until its next turn.
        host.write(w[a], new Uint8Array(memory, w[a + 1], w[a + 2]).slice());
        break;
      case op.ABORT: {
        const num = (i: u32) => BigInt(w[a + i]) | BigInt(w[a + i + 1]) << 32n;
        host.abort({ errno: w[a] | 0, line: w[a + 1], column: w[a + 2], a: num(3), b: num(5), file: utf8.decode(bytes_of(w, a + 7)) });
        break;
      }
      default:
        if ((code & op.PROGRAM) === 0) throw new InvariantError(Errno.ENOSYS);
        host.program(code, w, a);
    }
  });
}
