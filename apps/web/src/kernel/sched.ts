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
import { Command } from "@/kernel/abi/cmd.spec.ts";
import { CORE_BUILD, packet } from "@/kernel/ipc.ts";
import { CORE_KERNEL } from "@/kernel/mem.ts";
import { boot_block, quantum_policy } from "@/kernel/module.ts";

let driver: HTMLElement | null = null;

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
    packet((b) => b.f64(width).f64(height).f64(dpr()).f64(delta_s).f64(grant.frame_ms).bool(grant.interactive).u32(NET_CREDITS)),
  );
}, (run) => requestAnimationFrame(run));

export { strobe };
