// @pxl/client/host/mem.ts

/**
 * The opened guest and the host mounted on it (@xpute/host/host.ts). What
 * lies where in its memory is the guest's alone; others hold only addresses
 * it handed out.
 */

import type { Directory } from "@xpute/host/ipc/directory.ts";
import type { Doorbell } from "@xpute/host/ipc/doorbell.ts";
import { type Guest, type Host, mount, type Plan as Mount } from "@xpute/host/host.ts";
import { bracket, type Plan } from "@/bracket.tsx";

// Set once by `open_host`, which the boot awaits: Metro's bundle cannot wait on a module import.

export let HOST: Host;
export let GUEST: Guest;
/** A trapped turn leaves its errno and place in the host stream. */
export let GUEST_DOORBELL: Doorbell;
/** Where the guest says its rings and streams are, and its quantum. */
export let GUEST_DIRECTORY: Directory;

/** Opens the guest as `plan` says and mounts the host on it, its system calls answered by `sys` and each granted frame filled by `submit`. */
export async function open_host(plan: Plan, sys: Mount["sys"], submit: Mount["submit"]): Promise<void> {
  const guest = await bracket.guest.open(plan);
  HOST = mount(guest, { directory_at: plan.directory_at, sys, submit, next_frame: (run) => requestAnimationFrame(run) });
  GUEST = guest;
  GUEST_DOORBELL = HOST.doorbell;
  GUEST_DIRECTORY = HOST.directory;
}
