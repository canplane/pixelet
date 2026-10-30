// @xpute/host/host.ts

/**
 * A host mounted on a guest its platform opened: the directory, a doorbell
 * whose every turn runs the guest's host stream as it ends (a trapped turn's
 * too, so its report is read before the doorbell hears it stop), the system
 * calls and hostcalls answered through the program's `SysHost`, and the
 * strobe.
 */

import type { f64, i32, u32 } from "@xpute/kit/abi/word.ts";
import type { PacketWriter } from "@xpute/kit/wire/xtp/mod.ts";
import { Command, encode_sys_opened } from "@xpute/host/abi/cmd.spec.ts";
import { Directory } from "@xpute/host/ipc/directory.ts";
import { key } from "@xpute/host/ipc/directory.spec.ts";
import { Doorbell } from "@xpute/host/ipc/doorbell.ts";
import { Sys, type SysHost } from "@xpute/host/ipc/sys.ts";
import type { Grant } from "@xpute/host/sched/quantum.ts";
import { Strobe } from "@xpute/host/sched/strobe.ts";

/** The guest as its platform opened it: its memory, which never moves or
 * grows; its entry, `intr`, a turn that returns when it wants the next, in
 * ms; and where its hostcalls are answered. The floor names the entry, as a C
 * runtime names `main`. */
export interface Guest {
  mem: ArrayBuffer;
  intr(quota_ms: f64): f64;
  /** `answer` answers each hostcall from here on: a record at an offset of
   * `mem`, run before the hostcall returns. ENOSYS until it is set. */
  answer_with(answer: (at: u32) => i32): void;
}

/** What the program mounts its guest with. */
export interface Config {
  directory_at: u32;
  sys: SysHost;
  /** Fills the rings before each frame the strobe grants is rung. */
  submit(grant: Grant, delta_s: f64): void;
  next_frame(run: (now: f64) => void): void;
}

export interface Host {
  guest: Guest;
  directory: Directory;
  doorbell: Doorbell;
  strobe: Strobe;
  /** Sent with the next frame. */
  send(cmd: u32, pkt?: PacketWriter): void;
  /** Sent with the next frame, which it makes interactive. */
  input(cmd: u32, pkt?: PacketWriter): void;
  /** Rung at once with no quota, so it is heard after the strobe stopped too. */
  now(cmd: u32, pkt?: PacketWriter): void;
}

export function mount(guest: Guest, cfg: Config): Host {
  const directory = new Directory(guest.mem, cfg.directory_at);
  const stream = directory.get(key.SYS);
  const doorbell = new Doorbell(
    guest.mem,
    directory.get(key.SQ),
    directory.get(key.CQ),
    (quota_ms) => {
      let wake: f64;
      try {
        wake = guest.intr(quota_ms);
      } finally {
        sys.run(guest.mem, stream);
      }
      return wake;
    },
  );
  const strobe = new Strobe(
    doorbell,
    directory.quantum(),
    cfg.submit,
    cfg.next_frame,
  );
  const send = (cmd: u32, pkt?: PacketWriter) => {
    doorbell.send(0, cmd, pkt);
    strobe.request();
  };
  const sys = new Sys(cfg.sys, (fd, res, detail) =>
    send(Command.SYS_OPENED, encode_sys_opened(doorbell, fd, res, detail)),
  );
  guest.answer_with((at) => sys.hostcall(guest.mem, at));
  return {
    guest,
    directory,
    doorbell,
    strobe,
    send,
    input(cmd, pkt) {
      doorbell.send(0, cmd, pkt);
      strobe.input();
      strobe.request();
    },
    now(cmd, pkt) {
      doorbell.send(0, cmd, pkt);
      doorbell.ring(0);
      strobe.request();
    },
  };
}
