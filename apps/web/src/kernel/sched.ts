// @pxl/web/kernel/sched.ts

/**
 * The central scheduler's turns for the kernel (crates/xpute/runtime/src/sched).
 * A frame rings the doorbell with a quota after submitting what the frame
 * carries — the viewport, the gap, the frame interval as measured, the I/O
 * credits — and the return is when the kernel wants its next turn.
 *
 * The element is held here rather than passed in because the strobe fires from
 * the browser's frame and has to find it: what mounts it is the driver
 * (control/input.ts), and a turn with nothing mounted carries no frame.
 */

import type { f64 } from "@xpute/core/abi/word.ts";
import { Strobe } from "@xpute/runtime/sched/strobe.ts";
import { Command, encode_world_update } from "@/kernel/abi/cmd.spec.ts";
import { CORE_BUILD } from "@/kernel/ipc.ts";
import { CORE_KERNEL } from "@/kernel/mem.ts";
import { boot_block, quantum_policy } from "@/kernel/module.ts";

let driver: HTMLElement | null = null;

/** The local wall clock, as the frame's command carries it: unix ms, the
 * local hour, minute and second, and the local day's start. */
interface LocalTime {
  unix_ms: f64;
  hours: f64;
  minutes: f64;
  seconds: f64;
  day_start_ms: f64;
}

function local_time(): LocalTime {
  const d = new Date();
  const start = new Date(d);
  start.setHours(0, 0, 0, 0);
  return { unix_ms: d.getTime(), hours: d.getHours(), minutes: d.getMinutes(), seconds: d.getSeconds(), day_start_ms: start.getTime() };
}

/** Asks for a frame: the next animation frame strobes the kernel. */
export function request_frame(): void {
  strobe.request();
}

export function set_driver(element: HTMLElement | null): void {
  driver = element;
}

export function dpr(): f64 {
  return globalThis.devicePixelRatio || 1;
}

/** Fetches the kernel may have in flight at once: a browser opens about six
 * connections to an origin, and matching that keeps the kernel's queue — not
 * the socket pool — as what orders the work. */
const NET_CREDITS = 6;

/** The central scheduler's turns for the kernel, sized by the quantum the
 * kernel declared at boot; each frame carries the viewport it runs in. */
const strobe = new Strobe(CORE_BUILD, quantum_policy(boot_block(CORE_KERNEL)), (grant, delta_s) => {
  const element = driver;
  if (element === null) return;
  const width = Math.max(1, element.clientWidth);
  const height = Math.max(1, element.clientHeight);
  CORE_BUILD.send(
    0,
    Command.WORLD_UPDATE,
    encode_world_update({ width, height, dpr: dpr(), delta_s, frame_ms: grant.frame_ms, interactive: grant.interactive, credits: NET_CREDITS, ...local_time() }),
  );
}, (run) => requestAnimationFrame(run));

export { strobe };
