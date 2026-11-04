// @xpute/host/host.ts

/**
 * A host mounted on a guest its platform opened: the toc, a doorbell
 * whose every turn reads what the guest left as it ends (a trapped turn's
 * too, so its report is read before the doorbell hears it stop), the guest's
 * rings, on which the host sends as port 0 and the program's ports as
 * their own, the hostcalls, frames for port 0, answered through the
 * program's `SysHost`, and the
 * strobe, which runs turns as the scheduling policy the program boots it with
 * says (sched/policy.ts).
 */

import type { f64, i32, u32 } from "@xpute/kit/abi/word.ts";
import { strerror } from "@xpute/kit/status/errno.spec.ts";
import type { BranchCursor, PacketWriter } from "@xpute/kit/wire/xtp/mod.ts";
import { Command, encode_sys_opened } from "@xpute/host/abi/cmd.spec.ts";
import { Toc } from "@xpute/host/ipc/toc.ts";
import { key } from "@xpute/host/ipc/toc.spec.ts";
import * as frame from "@xpute/host/ipc/frame.ts";
import { Doorbell } from "@xpute/host/ipc/doorbell.ts";
import { Sys, type SysHost } from "@xpute/host/ipc/sys.ts";
import type { Grant, SchedulingPolicy } from "@xpute/host/sched/policy.ts";
import { Strobe } from "@xpute/host/sched/strobe.ts";

/** The guest as its platform opened it: its memory, which never moves or
 * grows; its entry, `run`, a turn that returns when it wants the next, in
 * ms; and where its hostcalls are answered. The floor names the entry, as a C
 * runtime names `main`. */
export interface Guest {
  mem: ArrayBuffer;
  run(quota_ms: f64): f64;
  /** `answer` answers each call from here on: a frame laid at an offset of
   * `mem`, answered before the call returns. ENOSYS until it is set. */
  answer_with(answer: (at: u32) => i32): void;
}

/** What the program mounts its guest with. */
export interface Config<G extends Grant = Grant> {
  toc_at: u32;
  sys: SysHost;
  /** When turns run and how much each gets, the program's policy. */
  policy: SchedulingPolicy<G>;
  /** Fills the rings before each turn the strobe grants is rung. */
  submit(grant: G, delta_s: f64): void;
}

export interface Host<G extends Grant = Grant> {
  guest: Guest;
  toc: Toc;
  doorbell: Doorbell;
  strobe: Strobe<G>;
  /** Sent as `src` with the next turn. */
  send(src: u32, cmd: u32, pkt?: PacketWriter): void;
  /** Sent as `src` and rung at once with no quota until the guest answers, so
   * it is heard after the strobe stopped too; then a turn is asked for. `take`
   * reads the answer; without it, the result is returned and a failure is
   * logged, as an answer no one reads is. */
  call(src: u32, cmd: u32, pkt?: PacketWriter | null): i32;
  call<T>(
    src: u32,
    cmd: u32,
    pkt: PacketWriter | null,
    take: (result: i32, reply: BranchCursor | null) => T,
  ): T;
  /** Told once the guest has stopped, after the strobe halted. */
  stopped: (() => void) | null;
}

export function mount<G extends Grant>(guest: Guest, cfg: Config<G>): Host<G> {
  const toc = new Toc(guest.mem, cfg.toc_at);
  const doorbell = new Doorbell(
    guest.mem,
    toc.get(key.SQ),
    toc.get(key.CQ),
    (quota_ms) => guest.run(quota_ms),
  );
  const strobe = new Strobe(doorbell, cfg.policy, cfg.submit);
  const send = (src: u32, cmd: u32, pkt?: PacketWriter) => {
    doorbell.send(src, cmd, pkt);
    strobe.request();
  };
  const sys = new Sys(cfg.sys, guest.mem, (fd, res, detail) =>
    send(
      frame.HOST,
      Command.SYS_OPENED,
      encode_sys_opened(doorbell, fd, res, detail),
    ),
  );
  doorbell.on(frame.HOST, sys.receive);
  guest.answer_with((at) => doorbell.answer(at));
  const host: Host<G> = {
    guest,
    toc,
    doorbell,
    strobe,
    send,
    call<T>(
      src: u32,
      cmd: u32,
      pkt: PacketWriter | null = null,
      take?: (result: i32, reply: BranchCursor | null) => T,
    ): T | i32 {
      const out = doorbell.call(src, cmd, pkt, (result, reply) => {
        if (take) return take(result, reply);
        if (result < 0) {
          console.warn(
            `[xpute/host] port ${src}'s command 0x${cmd.toString(16)} failed: ${strerror(result)}`,
          );
        }
        return result;
      });
      strobe.request();
      return out;
    },
    stopped: null,
  };
  doorbell.stopped = () => {
    strobe.halt();
    host.stopped?.();
  };
  return host;
}
