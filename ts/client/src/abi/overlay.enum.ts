// @pxl/client/abi/overlay.enum.ts
//
// GENERATED from spec/client/core/wit (xpute-link-gen wit.rs)

export type GroundStatus =
  | typeof GROUND_STATUS_IDLE
  | typeof GROUND_STATUS_LOADING
  | typeof GROUND_STATUS_SUCCESS;
export const GROUND_STATUS_IDLE = 0;
export const GROUND_STATUS_LOADING = 1;
export const GROUND_STATUS_SUCCESS = 2;

/** The member `val` is, or null for a number no member has. */
export function of_GroundStatus(val: number): GroundStatus | null {
  return val === GROUND_STATUS_IDLE ||
    val === GROUND_STATUS_LOADING ||
    val === GROUND_STATUS_SUCCESS
    ? val
    : null;
}

export type Message =
  typeof MESSAGE_WORLD_READ_FAILING | typeof MESSAGE_WORLD_UNAVAILABLE;
/** Reads reach the server and fail; they are tried again. */
export const MESSAGE_WORLD_READ_FAILING = 0;
/** Reads failed past every retry; the world is not coming. */
export const MESSAGE_WORLD_UNAVAILABLE = 1;

/** The member `val` is, or null for a number no member has. */
export function of_Message(val: number): Message | null {
  return val === MESSAGE_WORLD_READ_FAILING || val === MESSAGE_WORLD_UNAVAILABLE
    ? val
    : null;
}
