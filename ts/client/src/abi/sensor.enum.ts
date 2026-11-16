// @pxl/client/abi/sensor.enum.ts
//
// GENERATED from spec/client/core/wit (xpute-link-gen wit.rs)

export type LocationStatus =
  | typeof LOCATION_STATUS_IDLE
  | typeof LOCATION_STATUS_LOADING
  | typeof LOCATION_STATUS_READY
  | typeof LOCATION_STATUS_ERROR;
export const LOCATION_STATUS_IDLE = 0;
export const LOCATION_STATUS_LOADING = 1;
export const LOCATION_STATUS_READY = 2;
export const LOCATION_STATUS_ERROR = 3;

/** The member `val` is, or null for a number no member has. */
export function of_LocationStatus(val: number): LocationStatus | null {
  return val === LOCATION_STATUS_IDLE ||
    val === LOCATION_STATUS_LOADING ||
    val === LOCATION_STATUS_READY ||
    val === LOCATION_STATUS_ERROR
    ? val
    : null;
}

/** The member's name, as its source spells it. */
export function name_LocationStatus(val: LocationStatus): string {
  switch (val) {
    case LOCATION_STATUS_IDLE:
      return "Idle";
    case LOCATION_STATUS_LOADING:
      return "Loading";
    case LOCATION_STATUS_READY:
      return "Ready";
    case LOCATION_STATUS_ERROR:
      return "Error";
  }
}
