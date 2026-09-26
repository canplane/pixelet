// @pxl/web/kernel/mem.ts

/**
 * The core, opened, and its doorbell.
 *
 * How the core is opened is the platform's (platform/web/core.ts): a page
 * reserves a memory and instantiates the module over it, a device's app links
 * the library and is handed its memory. Either way what the shell holds is a
 * buffer and a doorbell, and what is where inside the buffer is the kernel's
 * alone (crates/pxl/web/kernel/src/mem/section.rs): the rings at addresses
 * the kernel's layout fixes, the fixed ranges past the bracket's front, and
 * the heap past them. The shell holds no picture of any of it — a block is
 * an address and a length the kernel handed out.
 */

import type { f64 } from "@xpute/core/abi/word.ts";
import { Doorbell } from "@xpute/runtime/ipc/doorbell.ts";
import { open_core } from "@platform/core.ts";
import { BOOT, boot_block, type Core } from "@/kernel/module.ts";
import { run_host_stream } from "@/kernel/host.ts";

// Set once by `load_core`, which the boot awaits before anything that reads
// them can run (kernel.ts `boot_kernel`): the core arrives over the network on
// a page, and a bundle that cannot wait on a module's import — Metro's —
// waits on the boot instead.

/** The core, booted: the kernel has laid the memory out before anything here
 * reads it. */
export let CORE: Core;

/** The rings to the core and its doorbell, made with it. A turn's falling
 * edge leaves what the kernel asks of the host in the host stream, run before
 * anything else can ring again — and where the core records its GPU calls
 * rather than making them, the platform's core runs those inside the turn it
 * was handed (platform/web/core.ts). A turn that traps leaves its report in
 * the host stream: the broken invariant's errno and place (xpute-core
 * status/bug.rs). */
export let CORE_DOORBELL: Doorbell;

/** Opens the core and makes its doorbell. A core that fails to open fails the
 * client — the memory exists for the kernel. */
export async function load_core(): Promise<void> {
  const core = await open_core();
  const boot = boot_block(core.buffer);
  CORE = core;
  CORE_DOORBELL = new Doorbell(core.buffer, boot[BOOT.sq], boot[BOOT.cq], (quota_ms) => {
    let wake: f64;
    try {
      wake = core.interrupt(quota_ms);
    } finally {
      run_host_stream(core.buffer, boot[BOOT.host]);
    }
    return wake;
  });
}
