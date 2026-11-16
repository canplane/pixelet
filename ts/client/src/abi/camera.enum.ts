// @pxl/client/abi/camera.enum.ts
//
// GENERATED from spec/client/core/wit (xpute-link-gen wit.rs)

export type Tracking =
  typeof TRACKING_RECENTER | typeof TRACKING_FOLLOW | typeof TRACKING_UNFOLLOW;
export const TRACKING_RECENTER = 0;
export const TRACKING_FOLLOW = 1;
export const TRACKING_UNFOLLOW = 2;

/** The member `val` is, or null for a number no member has. */
export function of_Tracking(val: number): Tracking | null {
  return val === TRACKING_RECENTER ||
    val === TRACKING_FOLLOW ||
    val === TRACKING_UNFOLLOW
    ? val
    : null;
}

/** The member's name, as its source spells it. */
export function name_Tracking(val: Tracking): string {
  switch (val) {
    case TRACKING_RECENTER:
      return "Recenter";
    case TRACKING_FOLLOW:
      return "Follow";
    case TRACKING_UNFOLLOW:
      return "Unfollow";
  }
}
