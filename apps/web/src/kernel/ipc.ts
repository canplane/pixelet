// @pxl/web/kernel/ipc.ts

/**
 * The shell's channel to the kernel: the doorbell over its rings
 * (@xpute/runtime/ipc/doorbell.ts) — commands sent, the turns rung, boot-time
 * queries answered, and the signals a turn leaves handed to whoever hears
 * them (kernel/kernel.ts). The kernel builds, schedules and draws its chunks
 * inside; none of that crosses here.
 */

export { packet } from "@xpute/runtime/ipc/doorbell.ts";

/** The one channel to the kernel: a live name for the doorbell `load_core`
 * makes, so that whoever imports it reads the one the boot made. */
export { CORE_DOORBELL as CORE_BUILD } from "@/kernel/mem.ts";
