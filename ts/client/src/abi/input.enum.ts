// @pxl/client/abi/input.enum.ts
//
// GENERATED from spec/client/core/wit (xpute-link-gen wit.rs)

export type PointerPhase =
  | typeof POINTER_PHASE_DOWN
  | typeof POINTER_PHASE_MOVE
  | typeof POINTER_PHASE_UP
  | typeof POINTER_PHASE_OUT
  | typeof POINTER_PHASE_CANCEL;
export const POINTER_PHASE_DOWN = 0;
export const POINTER_PHASE_MOVE = 1;
export const POINTER_PHASE_UP = 2;
export const POINTER_PHASE_OUT = 3;
/** The pointer's gesture ends with no tap: the platform took it, or the UI
 * spent it on something of its own. */
export const POINTER_PHASE_CANCEL = 4;

/** The member `val` is, or null for a number no member has. */
export function of_PointerPhase(val: number): PointerPhase | null {
  return val === POINTER_PHASE_DOWN ||
    val === POINTER_PHASE_MOVE ||
    val === POINTER_PHASE_UP ||
    val === POINTER_PHASE_OUT ||
    val === POINTER_PHASE_CANCEL
    ? val
    : null;
}

export type Drag = typeof DRAG_NONE | typeof DRAG_PAN | typeof DRAG_ROTATE;
export const DRAG_NONE = 0;
export const DRAG_PAN = 1;
export const DRAG_ROTATE = 2;

/** The member `val` is, or null for a number no member has. */
export function of_Drag(val: number): Drag | null {
  return val === DRAG_NONE || val === DRAG_PAN || val === DRAG_ROTATE
    ? val
    : null;
}
