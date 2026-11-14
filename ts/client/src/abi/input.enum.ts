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

export type PointerType =
  | typeof POINTER_TYPE_MOUSE
  | typeof POINTER_TYPE_TOUCH
  | typeof POINTER_TYPE_PEN;
export const POINTER_TYPE_MOUSE = 0;
export const POINTER_TYPE_TOUCH = 1;
export const POINTER_TYPE_PEN = 2;

/** The member `val` is, or null for a number no member has. */
export function of_PointerType(val: number): PointerType | null {
  return val === POINTER_TYPE_MOUSE ||
    val === POINTER_TYPE_TOUCH ||
    val === POINTER_TYPE_PEN
    ? val
    : null;
}
