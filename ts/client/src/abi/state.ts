// @pxl/client/abi/state.ts

/** The states the client reports beside the core, which the UI shows. They
 * are numbers both read, as a kernel's uapi headers are, so that neither side
 * reaches into the other. */

/**
 * Where the client's boot stands:
 * - opening: the core's module is fetched and compiled, before any GPU is
 *   asked for;
 * - starting: the core runs, while a GPU and the source are asked for;
 * - loading: a GPU answered and the source is declared, and the world is
 *   made under a cover;
 * - started: the app shows;
 * - failed: the client never started;
 * - terminated: the core stopped for good.
 */
export const BOOT_OPENING = 0;
export const BOOT_STARTING = 1;
export const BOOT_LOADING = 2;
export const BOOT_STARTED = 3;
export const BOOT_FAILED = 4;
export const BOOT_TERMINATED = 5;
export type Boot =
  | typeof BOOT_OPENING
  | typeof BOOT_STARTING
  | typeof BOOT_LOADING
  | typeof BOOT_STARTED
  | typeof BOOT_FAILED
  | typeof BOOT_TERMINATED;

/**
 * What the client has of a GPU. The core cannot say this itself, because a
 * core given no GPU learns nothing of one:
 * - asking: a GPU is asked for the first time;
 * - present: there is one;
 * - waiting: there is none yet, and one is asked for again;
 * - missing: there is none, and none will be asked for again;
 * - lost: there is none again, after one the core drew with;
 * - unsupported: this browser has no WebGPU;
 * - insecure: the page is not a secure context, so it has no GPU.
 */
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

/**
 * Where the world's source declaration stands:
 * - asking: it is asked for;
 * - failing: an ask was refused or went unanswered, and it is asked again;
 * - refused: it is refused for good, as one this client cannot read;
 * - declared: it is declared.
 */
export const SOURCE_ASKING = 0;
export const SOURCE_FAILING = 1;
export const SOURCE_REFUSED = 2;
export const SOURCE_DECLARED = 3;
export type Source =
  | typeof SOURCE_ASKING
  | typeof SOURCE_FAILING
  | typeof SOURCE_REFUSED
  | typeof SOURCE_DECLARED;
