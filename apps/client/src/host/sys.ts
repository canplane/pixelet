// @pxl/client/host/sys.ts

/**
 * The kernel's system calls as the host answers them, the `SysHost` that
 * `@xpute/host` hands every one to: an open and a write dispatched by root to
 * what is bound there (namespace/roots.ts), the program's own calls
 * (abi/text.ts), and the report that stopped the kernel. All run between turns,
 * so what they write is in place by the time the kernel reads it.
 */

import type { u32 } from "@xpute/kit/abi/word.ts";
import { strerror } from "@xpute/kit/status/errno.spec.ts";
import { Command as XputeCommand, encode_sys_opened } from "@xpute/host/abi/cmd.spec.ts";
import { Sys } from "@xpute/host/ipc/sys.ts";

import { commit_root, open_root } from "@/namespace/roots.ts";
import { GUEST_DOORBELL } from "@/host/mem.ts";
import { send_next } from "@/host/sched.ts";
import { text_call } from "@/abi/text.ts";
import { BUILD_NUMBER } from "@/config/deploy.ts";

let stopCode: string | null = null;

/** The kernel's last report, short as a stop code: its errno's name and the
 * file and line, a published build's file a number its paths decode by. None
 * where it stopped without one. */
export function stop_code(): string | null {
  return stopCode;
}

const SYS = new Sys({
  open: open_root,
  commit: commit_root,
  opened: (fd, res, detail) => send_next(XputeCommand.SYS_OPENED, encode_sys_opened(GUEST_DOORBELL, fd, res, detail)),
  abort: ({ errno, file, line, column, a, b }) => {
    console.error(`[kernel] stopped: ${strerror(errno)} at ${file.replace(/^\.+/, "")}:${line}:${column} (${a}, ${b})`);
    const where = `${file.split("/").pop()}:${line}`;
    stopCode = `${strerror(errno).split(":")[0]} · ${file.startsWith("#") ? `${where} · ${BUILD_NUMBER}` : where}`;
  },
  program: text_call,
});

export function run_host_stream(memory: ArrayBuffer, at: u32): void {
  SYS.run(memory, at);
}
