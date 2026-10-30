// @pxl/client/host/mount.ts

/**
 * The host mounted on the opened guest (@xpute/host/host.ts), and through it
 * the guest and its doorbell. What lies where in the guest's memory is the
 * guest's alone; others hold only addresses it handed out.
 */

import { type Config as Mount, type Host, mount } from "@xpute/host/host.ts";
import type {
  FrameGrant,
  FramePacing,
} from "@xpute/host/sched/frame_pacing.ts";
import { bracket, type Config } from "@/bracket.tsx";

/** Set once by `open_host`, which the boot awaits: Metro's bundle cannot wait on a module import. */
export let HOST: Host<FrameGrant>;

/** The scheduling policy the host was booted with, which an input makes interactive. */
export let PACING: FramePacing;

let view_open = false;
let page_visible = true;
let turning = false;

/** Turns run while the world's view is open and the page is seen: a hidden
 * page gets no frame, and a closed view draws none. */
function steer(): void {
  const turn = view_open && page_visible;
  if (turn === turning) return;
  turning = turn;
  if (turn) HOST.strobe.start();
  else HOST.strobe.stop();
}

/** The world's view opened or closed (port/input.ts `kernel_open`). */
export function set_view_open(open: boolean): void {
  view_open = open;
  steer();
}

/** The page or the app seen or hidden (driver/visibility.ts). */
export function set_page_visible(visible: boolean): void {
  page_visible = visible;
  steer();
}

/** Opens the guest as `cfg` says and mounts the host on it, its hostcalls answered by `sys`, its turns run as `policy` says and each filled by `submit`. */
export async function open_host(
  cfg: Config,
  sys: Mount["sys"],
  policy: FramePacing,
  submit: Mount<FrameGrant>["submit"],
): Promise<void> {
  const guest = await bracket.guest.open(cfg);
  PACING = policy;
  HOST = mount(guest, {
    directory_at: cfg.directory_at,
    sys,
    policy,
    submit,
  });
}
