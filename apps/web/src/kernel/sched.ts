// @pxl/web/kernel/sched.ts

/**
 * The central scheduler's turns for the kernel (crates/xpute/runtime/src/sched).
 * A frame rings the doorbell with a quota after submitting what the frame
 * carries — the viewport, the gap, the frame interval as measured, the I/O
 * credits — and the return is when the kernel wants its next turn.
 *
 * The viewport is held here rather than passed in because the strobe fires
 * from the frame and has to find it: what mounts the surface is the driver
 * (control/input.ts `kernel_open`, which each platform's surface calls), and
 * a turn with nothing mounted carries no frame.
 */

import type { U8Array } from "@xpute/core/abi/array.ts";
import type { f64, u32 } from "@xpute/core/abi/word.ts";
import { Strobe } from "@xpute/runtime/sched/strobe.ts";
import { Command, encode_world_frame } from "@/kernel/abi/cmd.spec.ts";
import { CORE, CORE_DOORBELL } from "@/kernel/mem.ts";
import { directory } from "@/kernel/module.ts";

/** The surface's size in CSS pixels, and the device pixels to one, as the
 * frame reads them: the platform's to say, since a page's window has a ratio
 * and a device's app has none of its own. */
export interface Viewport {
  width: f64;
  height: f64;
  dpr: f64;
}

/** Reads the mounted surface's size, asked every frame so a resize needs no
 * event of its own. */
let driver: (() => Viewport) | null = null;

/** Asks for a frame: the next animation frame strobes the kernel. */
export function request_frame(): void {
  strobe.request();
}

/** Sends a command for the next turn and asks for the frame that runs it. */
export function send_next(cmd: u32, pkt?: U8Array): void {
  CORE_DOORBELL.send(0, cmd, pkt);
  request_frame();
}

/** Sends an input for the next turn: the turns after it are interactive. */
export function send_input(cmd: u32, pkt?: U8Array): void {
  CORE_DOORBELL.send(0, cmd, pkt);
  strobe.input();
  request_frame();
}

/** Sends a command and rings it at once with no quota: what it asks of the
 * host is heard inside the call, the sensors' close sent after the strobe
 * has stopped included. */
export function ring_now(cmd: Command, pkt?: U8Array): void {
  CORE_DOORBELL.send(0, cmd, pkt);
  CORE_DOORBELL.ring(0);
  request_frame();
}

export function set_driver(viewport: (() => Viewport) | null): void {
  driver = viewport;
}

/** Fetches the kernel may have in flight at once: a browser opens about six
 * connections to an origin, and matching that keeps the kernel's queue — not
 * the socket pool — as what orders the work. */
const NET_CREDITS = 6;

/** The central scheduler's turns for the kernel, sized by the quantum the
 * kernel declared at boot; each frame carries the viewport it runs in. Made
 * by `start_sched` once the module has loaded. */
export let strobe: Strobe;

export function start_sched(): void {
  strobe = new Strobe(CORE_DOORBELL, directory(CORE.buffer).quantum(), (grant, delta_s) => {
    if (driver === null) return;
    const { width: w, height: h, dpr } = driver();
    const width = Math.max(1, w);
    const height = Math.max(1, h);
    // One instant: read as the wall clock, then moved to its day's start.
    const now = new Date();
    const unix_ms = now.getTime();
    const hours = now.getHours();
    const minutes = now.getMinutes();
    const seconds = now.getSeconds();
    now.setHours(0, 0, 0, 0);
    CORE_DOORBELL.send(
      0,
      Command.WORLD_FRAME,
      encode_world_frame({
        width,
        height,
        dpr,
        delta_s,
        frame_ms: grant.frame_ms,
        interactive: grant.interactive,
        credits: NET_CREDITS,
        unix_ms,
        hours,
        minutes,
        seconds,
        day_start_ms: now.getTime(),
      }),
    );
  }, (run) => requestAnimationFrame(run));
}
