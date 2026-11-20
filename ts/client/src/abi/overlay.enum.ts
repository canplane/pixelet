// @pxl/client/abi/overlay.enum.ts
//
// GENERATED from spec/client/core/wit (xpute-link-gen wit.rs)

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

/** The member's name, as its source spells it. */
export function name_Message(val: Message): string {
  switch (val) {
    case MESSAGE_WORLD_READ_FAILING:
      return "WorldReadFailing";
    case MESSAGE_WORLD_UNAVAILABLE:
      return "WorldUnavailable";
  }
}

export type CameraState =
  | typeof CAMERA_STATE_IDLE
  | typeof CAMERA_STATE_GESTURE
  | typeof CAMERA_STATE_ANIMATION;
/** Neither: any input is taken, and the camera mode can be changed. */
export const CAMERA_STATE_IDLE = 0;
/** A hand holds the view, or a key is held. What a gesture left to ease
 * goes on while idle. */
export const CAMERA_STATE_GESTURE = 1;
/** The camera's own way to a place it was sent to. */
export const CAMERA_STATE_ANIMATION = 2;

/** The member `val` is, or null for a number no member has. */
export function of_CameraState(val: number): CameraState | null {
  return val === CAMERA_STATE_IDLE ||
    val === CAMERA_STATE_GESTURE ||
    val === CAMERA_STATE_ANIMATION
    ? val
    : null;
}

/** The member's name, as its source spells it. */
export function name_CameraState(val: CameraState): string {
  switch (val) {
    case CAMERA_STATE_IDLE:
      return "Idle";
    case CAMERA_STATE_GESTURE:
      return "Gesture";
    case CAMERA_STATE_ANIMATION:
      return "Animation";
  }
}
