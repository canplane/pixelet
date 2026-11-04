// @pxl/client/abi/state.ts

/** What the client reports beside the kernel and the shell shows: numbers
 * both read, as a kernel's uapi headers are, so neither reaches into the other. */

/** Where the client's boot stands: starting, loading (the kernel runs, a GPU answered, the source declared: the world is made under a cover), started (the app shows), never started, or stopped for good. */
export const BOOT_STARTING = 0;
export const BOOT_LOADING = 1;
export const BOOT_STARTED = 2;
export const BOOT_FAILED = 3;
export const BOOT_STOPPED = 4;
export type Boot =
  | typeof BOOT_STARTING
  | typeof BOOT_LOADING
  | typeof BOOT_STARTED
  | typeof BOOT_FAILED
  | typeof BOOT_STOPPED;

/** What the client has of a GPU, which the guest, given none, cannot say: asked for the first time, one, none yet while it asks again, none it will ask for again, none again after one it drew with, no WebGPU in this browser, or none in a page that is not a secure context. */
export const GPU_ASKING = 0;
export const GPU_PRESENT = 1;
export const GPU_WAITING = 2;
export const GPU_MISSING = 3;
export const GPU_LOST = 4;
export const GPU_UNSUPPORTED = 5;
export const GPU_INSECURE = 6;
export type Gpu =
  | typeof GPU_ASKING
  | typeof GPU_PRESENT
  | typeof GPU_WAITING
  | typeof GPU_MISSING
  | typeof GPU_LOST
  | typeof GPU_UNSUPPORTED
  | typeof GPU_INSECURE;

/** Where the world's source declaration stands: asked, failing (refused or unanswered, and asked again), refused for good (one this client cannot read), or declared. */
export const SOURCE_ASKING = 0;
export const SOURCE_FAILING = 1;
export const SOURCE_REFUSED = 2;
export const SOURCE_DECLARED = 3;
export type Source =
  | typeof SOURCE_ASKING
  | typeof SOURCE_FAILING
  | typeof SOURCE_REFUSED
  | typeof SOURCE_DECLARED;
