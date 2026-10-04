// @pxl/web/kernel/mem.ts

/**
 * The opened core and its doorbell. What lies where in the buffer is the
 * kernel's alone; the shell holds addresses the kernel handed out.
 */

import type { f64 } from "@xpute/core/abi/word.ts";
import { Doorbell } from "@xpute/runtime/ipc/doorbell.ts";
import { openCore } from "@platform/core.ts";
import { key } from "@xpute/runtime/ipc/directory.spec.ts";
import { type Core, directory } from "@/kernel/module.ts";
import { run_host_stream } from "@/kernel/host.ts";

// Set once by `load_core`, which the boot awaits: Metro's bundle cannot wait on a module import.

export let CORE: Core;

/** A trapped turn leaves its errno and place in the host stream. */
export let CORE_DOORBELL: Doorbell;

export async function load_core(): Promise<void> {
  const core = await openCore();
  const dir = directory(core.buffer);
  const host_stream = dir.get(key.SYS);
  CORE = core;
  CORE_DOORBELL = new Doorbell(core.buffer, dir.get(key.SUBMISSION), dir.get(key.COMPLETION), (quota_ms) => {
    let wake: f64;
    try {
      wake = core.interrupt(quota_ms);
    } finally {
      run_host_stream(core.buffer, host_stream);
    }
    return wake;
  });
}
