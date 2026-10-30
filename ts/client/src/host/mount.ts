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

/** Opens the guest as `cfg` says and mounts the host on it, its system calls answered by `sys`, its turns run as `policy` says and each filled by `submit`. */
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
