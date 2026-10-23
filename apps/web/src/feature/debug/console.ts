// @pxl/web/feature/debug/console.ts

/**
 * A development page's console handle, `globalThis.pxl`: `pxl.cmd(name,
 * ...fields)` sends any kernel command by its lowercase name, its packet's
 * fields as its arguments (`pxl.cmd("input_view", dist, polar, azimuth)`;
 * the names and encoders are the spec's own table, `COMMANDS`); the page's
 * one clock (`pxl.clock(unix_s)`, the wall's again with none); and the
 * profiler's reads (`pxl.gauge(ms)`, `pxl.trace()`, `pxl.log(on)`). A check
 * is typed here, not carried in the page's URL.
 */

import { COMMANDS } from "@/kernel/abi/cmd.spec.ts";
import { set_clock } from "@/kernel/sched.ts";
import { kernel_command, kernel_room } from "@/kernel/shell.ts";
import type { PacketRoom, PacketWriter } from "@xpute/kit/wire/xtp/mod.ts";
import { profilerLog } from "./profiler/log.ts";
import { profilerTraceSave } from "./profiler/trace.ts";
import { profilerGaugeRead } from "./profiler/view.ts";

type Encode = (to: PacketRoom, ...fields: never[]) => PacketWriter;

export function installConsole(): void {
  const pxl: Record<string, unknown> = {};
  pxl.cmd = (name: keyof typeof COMMANDS, ...fields: never[]) => {
    const [code, encode] = COMMANDS[name];
    kernel_command(code, encode === null ? undefined : (encode as Encode)(kernel_room, ...fields));
  };
  pxl.clock = (unix_s?: number) => set_clock(unix_s === undefined ? () => Date.now() : () => unix_s * 1000);
  pxl.gauge = profilerGaugeRead;
  pxl.trace = profilerTraceSave;
  pxl.log = profilerLog;
  (globalThis as { pxl?: unknown }).pxl = pxl;
}
