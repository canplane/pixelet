// @pxl/client/kernel/mem.ts

/**
 * The opened guest and its doorbell. What lies where in the buffer is the
 * kernel's alone; the shell holds addresses the kernel handed out.
 */

import type { f64 } from "@xpute/kit/abi/word.ts";
import { Doorbell } from "@xpute/host/ipc/doorbell.ts";
import { openGuest } from "@platform/guest.ts";
import { key } from "@xpute/host/ipc/directory.spec.ts";
import { directory, type Guest } from "@/kernel/module.ts";
import { run_host_stream } from "@/kernel/host.ts";

// Set once by `load_guest`, which the boot awaits: Metro's bundle cannot wait on a module import.

export let GUEST: Guest;

/** A trapped turn leaves its errno and place in the host stream. */
export let GUEST_DOORBELL: Doorbell;

export async function load_guest(): Promise<void> {
  const guest = await openGuest();
  const dir = directory(guest.buffer);
  const host_stream = dir.get(key.SYS);
  GUEST = guest;
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
