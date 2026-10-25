// @pxl/client/host/mem.ts

/**
 * The opened guest, its directory and its doorbell. What lies where in the
 * buffer is the guest's alone; others hold only addresses it handed out.
 */

import type { f64 } from "@xpute/kit/abi/word.ts";
import type { Directory } from "@xpute/host/ipc/directory.ts";
import { Doorbell } from "@xpute/host/ipc/doorbell.ts";
import { type GuestPlan, openGuest } from "@/bracket.tsx";
import { key } from "@xpute/host/ipc/directory.spec.ts";
import { directory, type Guest } from "@/host/module.ts";
import { run_host_stream } from "@/host/sys.ts";

// Set once by `load_guest`, which the boot awaits: Metro's bundle cannot wait on a module import.

export let GUEST: Guest;

/** A trapped turn leaves its errno and place in the host stream. */
export let GUEST_DOORBELL: Doorbell;

/** Where the guest says its rings and streams are, and its quantum. */
export let GUEST_DIRECTORY: Directory;

/** Opens the guest as `plan` says, its directory at `plan.directory_at`. Each turn the
 * doorbell rings runs its host stream as it ends, a trapped one's too, so its
 * report is read before the doorbell hears it stop. */
export async function load_guest(plan: GuestPlan): Promise<void> {
  const guest = await openGuest(plan);
  const dir = directory(guest.buffer, plan.directory_at);
  const host_stream = dir.get(key.SYS);
  GUEST = guest;
  GUEST_DIRECTORY = dir;
  GUEST_DOORBELL = new Doorbell(guest.buffer, dir.get(key.SUBMISSION), dir.get(key.COMPLETION), (quota_ms) => {
    let wake: f64;
    try {
      wake = guest.interrupt(quota_ms);
    } finally {
      run_host_stream(guest.buffer, host_stream);
    }
    return wake;
  });
}
