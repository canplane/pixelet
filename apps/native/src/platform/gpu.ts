// @pxl/native/platform/gpu.ts

/**
 * The device the kernel draws with, on a device: the core's own, which it
 * opens through wgpu itself (crates/pxl/web/native). Nothing is asked for
 * from here, and nothing is recorded for the shell to run.
 */

export function gpu_boot(): void {}
