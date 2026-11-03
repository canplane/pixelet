// @pxl/client/abi/state.ts

/** What a driver or the client's boot reports and the shell shows: numbers
 * both read, as a kernel's uapi headers are, so neither reaches into the other. */

/** Where the client's boot stands: starting, loading (the kernel runs, a GPU answered, the source declared: the world is made under a cover), started (the app shows), never started, or stopped for good. */
export const enum Boot {
  Starting = 0,
  Loading = 1,
  Started = 2,
  Failed = 3,
  Stopped = 4,
}

/** What the host has of a GPU, which the guest, given none, cannot say: asked for the first time, one, none yet while it asks again, none it will ask for again, or none again after one it drew with. */
export const enum Gpu {
  Asking = 0,
  Present = 1,
  Waiting = 2,
  Missing = 3,
  Lost = 4,
}

/** Where the world's source declaration stands: asked, failing (refused or unanswered, and asked again), refused for good (one this client cannot read), or declared. */
export const enum Source {
  Asking = 0,
  Failing = 1,
  Refused = 2,
  Declared = 3,
}
