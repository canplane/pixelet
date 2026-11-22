// @pxl/client/abi/types.enum.ts
//
// GENERATED from spec/client/core/wit (xpute-link-gen wit.rs)

export type ErrorCode =
  | typeof ERROR_CODE_WOULD_BLOCK
  | typeof ERROR_CODE_BAD_DESCRIPTOR
  | typeof ERROR_CODE_BUSY
  | typeof ERROR_CODE_TOO_LONG
  | typeof ERROR_CODE_NOT_PERMITTED
  | typeof ERROR_CODE_INSUFFICIENT_MEMORY;
/** The settings are not read yet, and a change would be overwritten. */
export const ERROR_CODE_WOULD_BLOCK = 0;
/** No surface is open there. */
export const ERROR_CODE_BAD_DESCRIPTOR = 1;
/** The surface draws something else. */
export const ERROR_CODE_BUSY = 2;
/** The text is longer than its kind holds, a marker's or a chat line's. */
export const ERROR_CODE_TOO_LONG = 3;
/** The profiler's setting is off. */
export const ERROR_CODE_NOT_PERMITTED = 4;
/** The profiler's window could not be had. */
export const ERROR_CODE_INSUFFICIENT_MEMORY = 5;

/** The member `val` is, or null for a number no member has. */
export function of_ErrorCode(val: number): ErrorCode | null {
  return val === ERROR_CODE_WOULD_BLOCK ||
    val === ERROR_CODE_BAD_DESCRIPTOR ||
    val === ERROR_CODE_BUSY ||
    val === ERROR_CODE_TOO_LONG ||
    val === ERROR_CODE_NOT_PERMITTED ||
    val === ERROR_CODE_INSUFFICIENT_MEMORY
    ? val
    : null;
}

/** The member's name, as its source spells it. */
export function name_ErrorCode(val: ErrorCode): string {
  switch (val) {
    case ERROR_CODE_WOULD_BLOCK:
      return "WouldBlock";
    case ERROR_CODE_BAD_DESCRIPTOR:
      return "BadDescriptor";
    case ERROR_CODE_BUSY:
      return "Busy";
    case ERROR_CODE_TOO_LONG:
      return "TooLong";
    case ERROR_CODE_NOT_PERMITTED:
      return "NotPermitted";
    case ERROR_CODE_INSUFFICIENT_MEMORY:
      return "InsufficientMemory";
  }
}

export type LocationMode =
  | typeof LOCATION_MODE_OFF
  | typeof LOCATION_MODE_LOCATE
  | typeof LOCATION_MODE_TRACK;
export const LOCATION_MODE_OFF = 0;
export const LOCATION_MODE_LOCATE = 1;
export const LOCATION_MODE_TRACK = 2;

/** The member `val` is, or null for a number no member has. */
export function of_LocationMode(val: number): LocationMode | null {
  return val === LOCATION_MODE_OFF ||
    val === LOCATION_MODE_LOCATE ||
    val === LOCATION_MODE_TRACK
    ? val
    : null;
}

/** The member's name, as its source spells it. */
export function name_LocationMode(val: LocationMode): string {
  switch (val) {
    case LOCATION_MODE_OFF:
      return "Off";
    case LOCATION_MODE_LOCATE:
      return "Locate";
    case LOCATION_MODE_TRACK:
      return "Track";
  }
}
