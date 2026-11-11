// @xpute/runtime/abi/sys.spec.ts
//
// GENERATED from spec/abi/sys.json

/**
 * xpute's own protocol on majors NOP and SYS. Through it the runtime tells the core about
 * the core's life and its turns, and the core reports its end. NOP is the channel's own test.
 * SYS_ATTACH, SYS_SUSPEND and SYS_RESUME tell the core that a runtime attached, for the
 * first time or again, that the core is about to be frozen, and that it was woken; what
 * each one means the core keeps or resets is the core's to decide. SYS_TURN marks a turn
 * that the runtime's scheduling policy grants, and carries the wall clock; SYS_FRAME and
 * SYS_PACE carry frame pacing's grant and the core's asked rate. SYS_ABORT
 * reports the broken invariant the core stops on, the last frame the core sends.
 *
 * Each command is `major << 8 | minor` (io/frame.json). Its packet is an XTP branch whose
 * children are the command's fields, in order; a command with no fields carries no packet.
 * The GPU and the files are devices with protocols of their own (gpu.json, fs.json).
 * Majors 0x00 to 0x0F are xpute's (io/frame.json), and a program numbers the commands of
 * its own protocols past them.
 */

import type { BranchCursor } from "@xpute/kit/wire/xtp/mod.ts";
import type { f64, i32, u32 } from "@xpute/kit/abi/word.ts";
import { FINITE, INT_I32, INT_U32 } from "./number.ts";
import * as Major from "./sys/major.spec.ts";
import * as SysMinor from "./sys/sys_minor.spec.ts";
import * as Command from "./sys/command.spec.ts";
import * as Errno from "@xpute/kit/status/errno.spec.ts";
import { MarshalError } from "@xpute/kit/status/error.ts";
import type { Link, Protocol, OnImport } from "../mod.ts";
import { refusal_of } from "../mod.ts";

/** One of Major's, as its enum was a type. */
type Major = (typeof Major)[keyof typeof Major];
export { Major };

/** One of SysMinor's, as its enum was a type. */
type SysMinor = (typeof SysMinor)[keyof typeof SysMinor];
export { SysMinor };

/** One of Command's, as its enum was a type. */
type Command = (typeof Command)[keyof typeof Command];
export { Command };

/** Command.SYS_ABORT's packet. */
export interface SysAbort {
  errnum: i32;
  line: u32;
  column: u32;
  a_low: u32;
  a_high: u32;
  b_low: u32;
  b_high: u32;
  file: string;
}

/** Command.SYS_PACE's packet. */
export interface SysPace {
  interval_ms: f64;
}

/** Each import's packet, read from its branch, by its number. */
export const read = {
  [Command.SYS_ABORT]: (b: BranchCursor): SysAbort => ({
    errnum: INT_I32(b.at(0).get_number()),
    line: INT_U32(b.at(1).get_number()),
    column: INT_U32(b.at(2).get_number()),
    a_low: INT_U32(b.at(3).get_number()),
    a_high: INT_U32(b.at(4).get_number()),
    b_low: INT_U32(b.at(5).get_number()),
    b_high: INT_U32(b.at(6).get_number()),
    file: b.at(7).get_str(),
  }),
  [Command.SYS_PACE]: (b: BranchCursor): SysPace => ({
    interval_ms: FINITE(b.at(0).get_number()),
  }),
};

/** The sys protocol's exports as the UI calls them over `link`: a send returns
 * nothing, and a request a Promise of its reply's value, or of the memory its reply
 * names, which rejects with the errno it was refused with. `fits` says whether a
 * send's packet fits a frame. */
export function sys_exports(link: Link) {
  return {
    /** The channel's own test. The reply carries the packet's value, or 0 when the frame has no
     * packet. */
    nop: (val: u32) => link.send(Command.NOP, val),
    /** A runtime has attached, for the first time or after an earlier runtime went away. The
     * runtime sends it once it has laid the front of the core's memory (io/head.json). The core
     * lets go of whatever the earlier runtime's requests left behind. */
    sys_attach: () => link.send(Command.SYS_ATTACH),
    /** The core is about to be frozen: its turns stop for as long as its runtime is hidden. This
     * is the core's last sure moment to save. The UI sends it and does not wait; the runtime
     * lets it reach the core in a turn before turns stop. */
    sys_suspend: () => link.send(Command.SYS_SUSPEND),
    /** The core has been woken after SYS_SUSPEND. */
    sys_resume: () => link.send(Command.SYS_RESUME),
    /** Marks a turn that the scheduling policy grants, as opposed to a turn run only because a
     * command arrived. The runtime submits it after what the program submits for that turn,
     * and before it runs the turn. It carries the wall clock in Unix milliseconds. What frame pacing
     * says of the turn comes before it (SYS_FRAME); anything else the policy has to say, the
     * program sends in commands of its own. */
    sys_turn: (unix_ms: f64) => link.send(Command.SYS_TURN, unix_ms),
    /** The display frame frame pacing grants the next SYS_TURN at (@xpute/runtime
     * sched/frame_pacing.ts, xpute-runtime native/pacing.rs): the display's refresh, the interval
     * turns are paced at, a whole number of refreshes and the frame's budget, and whether the
     * turn follows an input. A runtime whose policy paces no display sends none. */
    sys_frame: (refresh_ms: f64, paced_ms: f64, interactive: boolean) =>
      link.send(Command.SYS_FRAME, refresh_ms, paced_ms, interactive),
    fits: {
      nop: (val: u32) => link.fits(Command.NOP, val),
      sys_attach: () => link.fits(Command.SYS_ATTACH),
      sys_suspend: () => link.fits(Command.SYS_SUSPEND),
      sys_resume: () => link.fits(Command.SYS_RESUME),
      sys_turn: (unix_ms: f64) => link.fits(Command.SYS_TURN, unix_ms),
      sys_frame: (refresh_ms: f64, paced_ms: f64, interactive: boolean) =>
        link.fits(Command.SYS_FRAME, refresh_ms, paced_ms, interactive),
    },
  };
}

/** The sys protocol's exports, as `sys_exports` binds them. */
export type SysExports = ReturnType<typeof sys_exports>;

/** The majors the runtime routes the protocol's imports by. */
export const MAJORS: readonly u32[] = [Major.SYS];

/** What the program does with each command the core posts on the sys protocol: a
 * call's member answers its value, and throws kit's error to refuse it. */
export interface SysImports {
  /** Reports the broken invariant the core stops on (xpute-kit status/bug.rs). */
  [Command.SYS_ABORT](e: SysAbort): void;
  /** The least interval the core asks between its turns, 0 for none: its load's word on the
   * rate, which the next SYS_FRAME's `paced_ms` answers, a whole number of refreshes. A policy
   * that paces no display ignores it. */
  [Command.SYS_PACE](e: SysPace): void;
}

/** `h` as the protocol the runtime routes the sys protocol's majors to: each
 * command read and handed to its member, whose refusal is answered with its
 * errno; ENOSYS for one the protocol does not carry. */
export function sys_imports(h: SysImports): Protocol {
  const on: OnImport = (_tag, cmd, f) => {
    try {
      switch (cmd) {
        case Command.SYS_ABORT:
          return h[Command.SYS_ABORT](read[Command.SYS_ABORT](need(f)));
        case Command.SYS_PACE:
          return h[Command.SYS_PACE](read[Command.SYS_PACE](need(f)));
        default:
          return Errno.ENOSYS;
      }
    } catch (err) {
      return refusal_of(err);
    }
  };
  return { majors: MAJORS, on };
}

/** A packet the command carries, which a frame without one breaks. */
function need<T>(f: T | null): T {
  if (f === null) throw new MarshalError(Errno.EBADMSG);
  return f;
}
