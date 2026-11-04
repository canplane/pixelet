// @pxl/client/host/mount.ts

/**
 * The host mounted on the opened guest (@xpute/host `mount`), and through it
 * the guest and its doorbell. What lies where in the guest's memory is the
 * guest's alone; others hold only addresses it handed out.
 */

import {
  type Config as Mount,
  type FrameGrant,
  type FramePacing,
  type Host,
  mount,
} from "@xpute/host";
import { bracket, type Config } from "@/bracket.tsx";

/** Set once by `open_host`, which the boot awaits: Metro's bundle cannot wait on a module import. */
export let HOST: Host;

/** The scheduling policy the host was booted with, which an input makes interactive. */
export let PACING: FramePacing;

let view_open = false;
let page_visible = true;

/** Turns run while the world's view is open and the page is seen: a hidden
 * page gets no frame, and a closed view draws none. */
function steer(): void {
  if (view_open && page_visible) HOST.resume();
  else HOST.pause();
}

/** The world's view opened or closed (port/input.ts `input.open`). */
export function set_view_open(open: boolean): void {
  view_open = open;
  steer();
}

/** The page or the app seen or hidden (port/sys.ts). */
export function set_page_visible(visible: boolean): void {
  page_visible = visible;
  steer();
}

/** Opens the guest as `cfg` says and mounts the host on it as `host_cfg`
 * says, its toc written in its first turn. */
export async function open_host(
  cfg: Config,
  host_cfg: Omit<Mount<FrameGrant>, "toc_at" | "toc_in_turn" | "policy"> & {
    policy: FramePacing;
  },
): Promise<void> {
  const guest = await bracket.guest.open(cfg);
  PACING = host_cfg.policy;
  HOST = mount(guest, { ...host_cfg, toc_at: cfg.toc_at, toc_in_turn: true });
}
