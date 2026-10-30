// @xpute/host/host.ts

/**
 * A host mounted on a guest its platform opened: the directory, a doorbell
 * whose every turn runs the guest's host stream as it ends (a trapped turn's
 * too, so its report is read before the doorbell hears it stop), the guest's
 * rings, on which the host sends as port 0 and the program's ports as
 * their own, the hostcalls answered through the program's `SysHost`, and the
 * strobe, which runs turns as the scheduling policy the program boots it with
 * says (sched/policy.ts).
 */

import type { f64, i32, u32 } from "@xpute/kit/abi/word.ts";
import type { PacketWriter } from "@xpute/kit/wire/xtp/mod.ts";
import { Command, encode_sys_opened } from "@xpute/host/abi/cmd.spec.ts";
import { Directory } from "@xpute/host/ipc/directory.ts";
import { key } from "@xpute/host/ipc/directory.spec.ts";
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
  /** `answer` answers each hostcall from here on: a record at an offset of
   * `mem`, run before the hostcall returns. ENOSYS until it is set. */
  answer_with(answer: (at: u32) => i32): void;
}

/** What the program mounts its guest with. */
export interface Config<G extends Grant = Grant> {
  directory_at: u32;
  sys: SysHost;
  /** When turns run and how much each gets, the program's policy. */
  policy: SchedulingPolicy<G>;
  /** Fills the rings before each turn the strobe grants is rung. */
  submit(grant: G, delta_s: f64): void;
}

export interface Host<G extends Grant = Grant> {
  guest: Guest;
  directory: Directory;
  doorbell: Doorbell;
  strobe: Strobe<G>;
  /** Sent as `src` with the next turn. */
  send(src: u32, cmd: u32, pkt?: PacketWriter): void;
  /** Sent as `src` and rung at once with no quota, so it is heard after the
   * strobe stopped too. */
  now(src: u32, cmd: u32, pkt?: PacketWriter): void;
}

export function mount<G extends Grant>(guest: Guest, cfg: Config<G>): Host<G> {
  const directory = new Directory(guest.mem, cfg.directory_at);
  const stream = directory.get(key.SYS);
  const doorbell = new Doorbell(
    guest.mem,
    directory.get(key.SQ),
    directory.get(key.CQ),
    (quota_ms) => {
      let wake: f64;
      try {
        wake = guest.run(quota_ms);
      } finally {
        sys.run(guest.mem, stream);
      }
      return wake;
    },
  );
  const strobe = new Strobe(doorbell, cfg.policy, cfg.submit);
  const send = (src: u32, cmd: u32, pkt?: PacketWriter) => {
    doorbell.send(src, cmd, pkt);
    strobe.request();
  };
  const sys = new Sys(cfg.sys, (fd, res, detail) =>
    send(
      frame.HOST,
      Command.SYS_OPENED,
      encode_sys_opened(doorbell, fd, res, detail),
    ),
  );
  guest.answer_with((at) => sys.call_now(guest.mem, at));
  return {
    guest,
    directory,
    doorbell,
    strobe,
    send,
    now(src, cmd, pkt) {
      doorbell.send(src, cmd, pkt);
      doorbell.ring(0);
      strobe.request();
    },
  };
}
