// @pxl/client/host/sys.ts

/**
 * The guest's system calls as the host answers them: `@xpute/host`'s `Sys`
 * over the `SysHost` the boot hands in, which says what an open, a write, a
 * program call and the report that stopped the guest mean. The host answers
 * an open itself (`SYS_OPENED`). All run between turns, so what they write is
 * in place by the time the guest reads it.
 */

import type { u32 } from "@xpute/kit/abi/word.ts";
import { Command as XputeCommand, encode_sys_opened } from "@xpute/host/abi/cmd.spec.ts";
import { Sys, type SysHost } from "@xpute/host/ipc/sys.ts";

import { GUEST_DOORBELL } from "@/host/mem.ts";
import { send_next } from "@/host/sched.ts";

let SYS: Sys | null = null;

/** Once, before the guest's first turn is rung. */
export function start_sys(host: Omit<SysHost, "opened">): void {
  if (SYS !== null) throw new Error("sys: started twice");
  SYS = new Sys({ ...host, opened: (fd, res, detail) => send_next(XputeCommand.SYS_OPENED, encode_sys_opened(GUEST_DOORBELL, fd, res, detail)) });
}

export function run_host_stream(memory: ArrayBuffer, at: u32): void {
  SYS!.run(memory, at);
}
