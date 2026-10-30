// @pxl/client/abi/state.ts

/** What a driver reports and the shell shows: numbers both read, as a
 * kernel's uapi headers are, so neither reaches into the other. */

/** What the host has of a GPU, which the guest, given none, cannot say: one, none yet while it asks again, none it will ask for again, or none again after one it drew with. */
export const enum Gpu {
  Present = 0,
  Waiting = 1,
  Missing = 2,
  Lost = 3,
}

/** Where the world's source declaration stands: asked, failing (refused or unanswered, and asked again), refused for good (one this client cannot read), or declared. */
export const enum Source {
  Asking = 0,
  Failing = 1,
  Refused = 2,
  Declared = 3,
}
