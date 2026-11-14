// @pxl/client/abi/sensor.enum.ts
//
// GENERATED from spec/client/core/wit (xpute-link-gen wit.rs)

export type GeolocationErrorCode =
  | typeof GEOLOCATION_ERROR_CODE_OTHER
  | typeof GEOLOCATION_ERROR_CODE_PERMISSION_DENIED
  | typeof GEOLOCATION_ERROR_CODE_POSITION_UNAVAILABLE
  | typeof GEOLOCATION_ERROR_CODE_TIMEOUT;
export const GEOLOCATION_ERROR_CODE_OTHER = 0;
export const GEOLOCATION_ERROR_CODE_PERMISSION_DENIED = 1;
export const GEOLOCATION_ERROR_CODE_POSITION_UNAVAILABLE = 2;
export const GEOLOCATION_ERROR_CODE_TIMEOUT = 3;

/** The member `val` is, or null for a number no member has. */
export function of_GeolocationErrorCode(
  val: number,
): GeolocationErrorCode | null {
  return val === GEOLOCATION_ERROR_CODE_OTHER ||
    val === GEOLOCATION_ERROR_CODE_PERMISSION_DENIED ||
    val === GEOLOCATION_ERROR_CODE_POSITION_UNAVAILABLE ||
    val === GEOLOCATION_ERROR_CODE_TIMEOUT
    ? val
    : null;
}

export type PermissionAnswer =
  | typeof PERMISSION_ANSWER_ASKING
  | typeof PERMISSION_ANSWER_GRANTED
  | typeof PERMISSION_ANSWER_DENIED
  | typeof PERMISSION_ANSWER_FAILED;
export const PERMISSION_ANSWER_ASKING = 0;
export const PERMISSION_ANSWER_GRANTED = 1;
export const PERMISSION_ANSWER_DENIED = 2;
export const PERMISSION_ANSWER_FAILED = 3;

/** The member `val` is, or null for a number no member has. */
export function of_PermissionAnswer(val: number): PermissionAnswer | null {
  return val === PERMISSION_ANSWER_ASKING ||
    val === PERMISSION_ANSWER_GRANTED ||
    val === PERMISSION_ANSWER_DENIED ||
    val === PERMISSION_ANSWER_FAILED
    ? val
    : null;
}
