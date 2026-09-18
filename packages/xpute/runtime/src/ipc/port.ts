// @xpute/runtime/ipc/port.ts

/**
 * A command table: codec and dispatch, nothing about transport.
 *
 * A handler reads its args and body from the frame, writes its reply into
 * the same frame, and returns a spill if its body did not fit. It never
 * sets `RES`, never allocates a tx_id and never decides whether a reply is
 * sent — that is the runtime's (link.ts#serve) — which is what lets the same
 * table sit behind a worker, an in-memory link or, later, a WASM export.
 * The contract is April's CommandPort's, with the command numeric.
 */

import type { u32 } from "@xpute/core/kit/word.ts";
import { Errno } from "@xpute/core/errno.ts";
import { XputeError } from "@xpute/core/error.ts";

import { Frame, type FrameMessage } from "./frame.ts";

/** Handles one request in place. Returns the reply's spill, or null. */
export type CommandHandler = (frame: Frame, spill: ArrayBuffer | null) => ArrayBuffer | null;

export class CommandPort {
  private readonly handlers: (CommandHandler | undefined)[] = [];

  on(cmd: u32, handler: CommandHandler): void {
    if (this.handlers[cmd]) throw new Error(`ipc: command ${cmd} registered twice`);
    this.handlers[cmd] = handler;
  }

  /**
   * Runs the request's handler and returns the reply message, the frame
   * rewritten in place. An unknown command replies ENOENT; a handler that
   * throws replies with the XputeError's errno, or EIO for anything else,
   * and an empty reply — never an exception across the boundary.
   */
  dispatch(msg: FrameMessage): FrameMessage {
    const frame = new Frame(msg.frame);
    const handler = this.handlers[frame.cmd];
    let spill: ArrayBuffer | null = null;
    let errno: Errno = 0;
    if (!handler) {
      errno = Errno.ENOENT;
    } else {
      try {
        spill = handler(frame, msg.spill);
      } catch (err) {
        errno = err instanceof XputeError ? err.errno : Errno.EIO;
        if (!(err instanceof XputeError)) console.error(`[ipc] command ${frame.cmd} failed:`, err);
      }
    }
    if (errno !== 0) {
      frame.lay_out(0, 0);
      spill = null;
    }
    frame.errno = errno;
    return { frame: msg.frame, spill };
  }
}
