// @pxl/web/kernel/ipc.ts

/**
 * The shell's channel to the kernel: the doorbell over its rings
 * (@xpute/runtime/ipc/doorbell.ts) — commands sent, the turns rung, boot-time
 * queries answered, and the signals a turn leaves handed to whoever hears
 * them (kernel/kernel.ts). The kernel builds, schedules and draws its chunks
 * inside; none of that crosses here.
 */

import type { Doorbell } from "@xpute/runtime/ipc/doorbell.ts";

import { CORE_DOORBELL } from "@/kernel/mem.ts";

export { packet } from "@xpute/runtime/ipc/doorbell.ts";

/** The one channel to the kernel. */
export const CORE_BUILD: Doorbell = CORE_DOORBELL;
