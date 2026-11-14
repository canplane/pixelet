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

export type SensorStatus =
  | typeof SENSOR_STATUS_IDLE
  | typeof SENSOR_STATUS_LOADING
  | typeof SENSOR_STATUS_READY
  | typeof SENSOR_STATUS_ERROR
  | typeof SENSOR_STATUS_UNAVAILABLE;
export const SENSOR_STATUS_IDLE = 0;
export const SENSOR_STATUS_LOADING = 1;
export const SENSOR_STATUS_READY = 2;
export const SENSOR_STATUS_ERROR = 3;
export const SENSOR_STATUS_UNAVAILABLE = 4;

/** The member `val` is, or null for a number no member has. */
export function of_SensorStatus(val: number): SensorStatus | null {
  return val === SENSOR_STATUS_IDLE ||
    val === SENSOR_STATUS_LOADING ||
    val === SENSOR_STATUS_READY ||
    val === SENSOR_STATUS_ERROR ||
    val === SENSOR_STATUS_UNAVAILABLE
    ? val
    : null;
}

export type Message =
  | typeof MESSAGE_GEOLOCATION_UNSUPPORTED
  | typeof MESSAGE_GEOLOCATION_PERMISSION_DENIED
  | typeof MESSAGE_GEOLOCATION_POSITION_UNAVAILABLE
  | typeof MESSAGE_GEOLOCATION_TIMEOUT
  | typeof MESSAGE_GEOLOCATION_GENERIC_ERROR
  | typeof MESSAGE_WORLD_READ_FAILING
  | typeof MESSAGE_WORLD_UNAVAILABLE;
export const MESSAGE_GEOLOCATION_UNSUPPORTED = 0;
export const MESSAGE_GEOLOCATION_PERMISSION_DENIED = 1;
export const MESSAGE_GEOLOCATION_POSITION_UNAVAILABLE = 2;
export const MESSAGE_GEOLOCATION_TIMEOUT = 3;
export const MESSAGE_GEOLOCATION_GENERIC_ERROR = 4;
/** Reads reach the server and fail; they are tried again. */
export const MESSAGE_WORLD_READ_FAILING = 5;
/** Reads failed past every retry; the world is not coming. */
export const MESSAGE_WORLD_UNAVAILABLE = 6;

/** The member `val` is, or null for a number no member has. */
export function of_Message(val: number): Message | null {
  return val === MESSAGE_GEOLOCATION_UNSUPPORTED ||
    val === MESSAGE_GEOLOCATION_PERMISSION_DENIED ||
    val === MESSAGE_GEOLOCATION_POSITION_UNAVAILABLE ||
    val === MESSAGE_GEOLOCATION_TIMEOUT ||
    val === MESSAGE_GEOLOCATION_GENERIC_ERROR ||
    val === MESSAGE_WORLD_READ_FAILING ||
    val === MESSAGE_WORLD_UNAVAILABLE
    ? val
    : null;
}
