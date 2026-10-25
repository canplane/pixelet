// @pxl/client/host/mount.ts

/**
 * The host mounted on the opened guest (@xpute/host/host.ts), and through it
 * the guest and its doorbell. What lies where in the guest's memory is the
 * guest's alone; others hold only addresses it handed out.
 */

import { type Host, mount, type Plan as Mount } from "@xpute/host/host.ts";
import { bracket, type Plan } from "@/bracket.tsx";

/** Set once by `open_host`, which the boot awaits: Metro's bundle cannot wait on a module import. */
export let HOST: Host;

/** Opens the guest as `plan` says and mounts the host on it, its system calls answered by `sys` and each granted frame filled by `submit`. */
export async function open_host(
  plan: Plan,
  sys: Mount["sys"],
  submit: Mount["submit"],
): Promise<void> {
  const guest = await bracket.guest.open(plan);
  HOST = mount(guest, {
    directory_at: plan.directory_at,
    sys,
    submit,
    next_frame: (run) => requestAnimationFrame(run),
  });
}
